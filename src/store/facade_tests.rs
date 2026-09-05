//! Tests for the [`MetricStore`](super::MetricStore) facade: opening, error
//! paths, and the ingest-query-retention cycle across modules.
//!
//! Kept out of `mod.rs` so the public surface stays readable in one screen.

#![allow(clippy::unwrap_used, clippy::expect_used)]

mod tests {
    use super::super::*;
    use std::io::Write;

    fn sample(host_id: i64, ts: i64, cpu: f64) -> MetricSample {
        let mut sample = MetricSample::new(host_id, ts);
        sample.cpu_percent = Some(cpu);
        sample
    }

    #[test]
    fn in_memory_store_is_ready_to_use() {
        let store = MetricStore::open_in_memory().unwrap();
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        assert!(store.path().is_none());
        assert!(store.hosts().unwrap().is_empty());
    }

    #[test]
    fn open_creates_missing_directories_and_the_schema() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("nested").join("deeper").join("ratterm.db");
        let store = MetricStore::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        assert_eq!(store.path(), Some(path.as_path()));
        assert!(path.exists(), "the database file should exist on disk");
    }

    #[test]
    fn reopening_an_existing_database_keeps_its_rows() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("ratterm.db");

        let host_id = {
            let mut store = MetricStore::open(&path).unwrap();
            let host_id = store.register_host("cthulhu-computer", 1_000).unwrap();
            store.record_sample(&sample(host_id, 1_000, 12.5)).unwrap();
            store.record_sample(&sample(host_id, 1_060, 25.0)).unwrap();
            host_id
        };

        let store = MetricStore::open(&path).unwrap();
        assert_eq!(store.schema_version().unwrap(), SCHEMA_VERSION);
        let rows = store.samples_between(host_id, 0, 10_000).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].cpu_percent, Some(12.5));
        let hosts = store.hosts().unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].label.as_deref(), Some("cthulhu-computer"));
    }

    #[test]
    fn opening_where_the_directory_cannot_be_created_reports_the_path() {
        let dir = tempfile::tempdir().unwrap();
        let blocker = dir.path().join("blocker");
        std::fs::write(&blocker, b"not a directory").unwrap();
        // `blocker` is a file, so creating `blocker/sub` cannot succeed.
        let path = blocker.join("sub").join("ratterm.db");

        let err = MetricStore::open(&path).unwrap_err();
        match err {
            StoreError::Directory { path: reported, .. } => {
                assert!(reported.ends_with("sub"), "got {reported:?}");
            }
            other => panic!("expected a Directory error, got {other:?}"),
        }
    }

    #[test]
    fn opening_a_file_that_is_not_a_database_fails() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("garbage.db");
        let mut file = std::fs::File::create(&path).unwrap();
        // Enough bytes that SQLite reads a full page one and rejects it.
        file.write_all(&vec![0xA5_u8; 8192]).unwrap();
        file.sync_all().unwrap();
        drop(file);

        let err = MetricStore::open(&path).unwrap_err();
        assert!(
            matches!(err, StoreError::Sqlite(_)),
            "expected a sqlite error, got {err:?}"
        );
    }

    #[test]
    fn a_database_from_a_newer_build_is_refused() {
        let dir = tempfile::tempdir().unwrap();
        let path = dir.path().join("future.db");
        {
            let store = MetricStore::open(&path).unwrap();
            store
                .conn
                .pragma_update(None, "user_version", SCHEMA_VERSION + 1)
                .unwrap();
        }
        let err = MetricStore::open(&path).unwrap_err();
        assert!(matches!(err, StoreError::SchemaVersion { .. }), "{err:?}");
    }

    #[test]
    fn ingest_writes_the_sample_and_the_alert_together() {
        let mut store = MetricStore::open_in_memory().unwrap();
        let rules = [AlertRule::new(
            AlertMetric::CpuPercent,
            Comparison::Above,
            90.0,
        )];
        let host = store.register_host("gpu-box", 0).unwrap();

        let update = store.ingest(&sample(host, 10, 95.0), &rules).unwrap();
        assert_eq!(update.fired.len(), 1);
        assert_eq!(store.latest_sample(host).unwrap().unwrap().ts, 10);
        assert_eq!(store.active_alerts(host).unwrap().len(), 1);

        let calm = store.ingest(&sample(host, 20, 10.0), &rules).unwrap();
        assert_eq!(calm.cleared.len(), 1);
        assert!(store.active_alerts(host).unwrap().is_empty());
        assert_eq!(store.recent_alerts(10).unwrap().len(), 1);
    }

    #[test]
    fn a_full_ingest_query_and_retention_cycle() {
        let mut store = MetricStore::open_in_memory().unwrap();
        let host = store.register_host("rock-5c", 0).unwrap();

        let samples: Vec<MetricSample> = (0..300)
            .map(|i| sample(host, i * 10, (i % 100) as f64))
            .collect();
        assert_eq!(store.record_samples(&samples).unwrap(), 300);

        let summary = store.summary(host, 0, 3_000).unwrap();
        assert_eq!(summary.sample_count, 300);
        assert_eq!(summary.stats(MetricKind::CpuPercent).unwrap().max, 99.0);

        let series = store
            .sparkline_series(host, MetricKind::CpuPercent, 0, 2_999, 30)
            .unwrap();
        assert_eq!(series.len(), 30);
        assert!(series.iter().all(|v| v.is_some()));

        assert_eq!(store.offline_since(host).unwrap(), Some(2_990));

        let report = store
            .downsample(1_000_000, &RetentionPolicy::default())
            .unwrap();
        assert_eq!(report.raw_collapsed, 300);
        assert_eq!(report.minute_written, 50);
        assert!(!report.is_empty());

        let rows = store.samples_between(host, 0, 3_000).unwrap();
        assert_eq!(rows.len(), 50);
        assert!(rows.iter().all(|r| r.resolution == RESOLUTION_MINUTE));
    }

    #[test]
    fn events_and_sessions_round_trip_through_the_facade() {
        let mut store = MetricStore::open_in_memory().unwrap();
        let host = store.register_host("ubuntu-box", 0).unwrap();

        store
            .record_container_event(&ContainerEvent {
                event_id: None,
                host_id: host,
                container_id: "c1".to_string(),
                container_name: Some("api".to_string()),
                ts: 50,
                event: "start".to_string(),
                detail: None,
            })
            .unwrap();
        assert_eq!(
            store.container_events_between(host, 0, 100).unwrap().len(),
            1
        );

        store
            .record_k8s_event(&K8sEvent {
                event_id: None,
                context: "prod".to_string(),
                namespace: "default".to_string(),
                kind: "Pod".to_string(),
                name: "api-0".to_string(),
                ts: 60,
                reason: Some("Killing".to_string()),
                message: None,
            })
            .unwrap();
        assert_eq!(
            store
                .k8s_events_between("prod", None, 0, 100)
                .unwrap()
                .len(),
            1
        );

        store
            .index_log_file(&LogIndexEntry {
                entry_id: None,
                host_id: host,
                container_id: "c1".to_string(),
                date: "2026-09-01".to_string(),
                path: "/var/log/c1.log".to_string(),
                line_count: 10,
                byte_size: 800,
            })
            .unwrap();
        assert_eq!(
            store
                .log_files_between(host, "c1", "2026-09-01", "2026-09-30")
                .unwrap()
                .len(),
            1
        );

        let session = store.start_session(1_000).unwrap();
        store.end_session(session, 2_000, &[host]).unwrap();
        let sessions = store.recent_sessions(5).unwrap();
        assert_eq!(sessions[0].hosts_touched, vec![host]);
    }

    #[test]
    fn debug_shows_the_path_without_the_connection() {
        let store = MetricStore::open_in_memory().unwrap();
        let text = format!("{store:?}");
        assert!(text.contains("MetricStore"));
        assert!(text.contains("path"));
    }
}
