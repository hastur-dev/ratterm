//! Discrete events: container lifecycle, Kubernetes events, application
//! sessions, and the Docker log file index.
//!
//! These share a file because they are all append-mostly tables with no
//! aggregation, unlike the metric series.

use rusqlite::{Connection, params};

use super::error::StoreError;
use super::metrics::touch_host;
use super::types::{ContainerEvent, K8sEvent, LogIndexEntry, SessionRecord};

/// Records a Docker lifecycle event and returns its row id.
///
/// The host is registered first because `container_events.host_id` is a
/// foreign key and a container event can arrive before the first metric
/// sample from a newly discovered host.
pub fn record_container_event(
    conn: &Connection,
    event: &ContainerEvent,
) -> Result<i64, StoreError> {
    touch_host(conn, event.host_id, event.ts)?;
    conn.execute(
        "INSERT INTO container_events \
             (host_id, container_id, container_name, ts, event, detail) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6)",
        params![
            event.host_id,
            event.container_id,
            event.container_name,
            event.ts,
            event.event,
            event.detail,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Container events for one host in `[from_ts, to_ts]`, oldest first.
pub fn container_events_between(
    conn: &Connection,
    host_id: i64,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<ContainerEvent>, StoreError> {
    let mut stmt = conn.prepare_cached(
        "SELECT event_id, host_id, container_id, container_name, ts, event, detail \
         FROM container_events \
         WHERE host_id = ?1 AND ts >= ?2 AND ts <= ?3 \
         ORDER BY ts, event_id",
    )?;
    let rows = stmt.query_map(params![host_id, from_ts, to_ts], |row| {
        Ok(ContainerEvent {
            event_id: row.get(0)?,
            host_id: row.get(1)?,
            container_id: row.get(2)?,
            container_name: row.get(3)?,
            ts: row.get(4)?,
            event: row.get(5)?,
            detail: row.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Records a Kubernetes event and returns its row id.
///
/// Kubernetes events are not tied to a host row: the same cluster is reached
/// from any machine, and the context name is the stable identity.
pub fn record_k8s_event(conn: &Connection, event: &K8sEvent) -> Result<i64, StoreError> {
    conn.execute(
        "INSERT INTO k8s_events (context, namespace, kind, name, ts, reason, message) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7)",
        params![
            event.context,
            event.namespace,
            event.kind,
            event.name,
            event.ts,
            event.reason,
            event.message,
        ],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Kubernetes events for one context in `[from_ts, to_ts]`, oldest first.
///
/// Passing `None` for `namespace` returns every namespace in the context.
pub fn k8s_events_between(
    conn: &Connection,
    context: &str,
    namespace: Option<&str>,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<K8sEvent>, StoreError> {
    let mut stmt = conn.prepare_cached(
        "SELECT event_id, context, namespace, kind, name, ts, reason, message \
         FROM k8s_events \
         WHERE context = ?1 AND (?2 IS NULL OR namespace = ?2) \
           AND ts >= ?3 AND ts <= ?4 \
         ORDER BY ts, event_id",
    )?;
    let rows = stmt.query_map(params![context, namespace, from_ts, to_ts], |row| {
        Ok(K8sEvent {
            event_id: row.get(0)?,
            context: row.get(1)?,
            namespace: row.get(2)?,
            kind: row.get(3)?,
            name: row.get(4)?,
            ts: row.get(5)?,
            reason: row.get(6)?,
            message: row.get(7)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Parses a `YYYY-MM-DD` index date into midnight UTC.
///
/// The format is validated here rather than trusted, because the date is the
/// key a range search compares against as text; a differently shaped string
/// would sort into the wrong place and silently hide log files.
fn date_to_ts(date: &str) -> Result<i64, StoreError> {
    let parsed = chrono::NaiveDate::parse_from_str(date, "%Y-%m-%d")
        .map_err(|e| StoreError::InvalidArgument(format!("log index date {date:?}: {e}")))?;
    let midnight = parsed.and_hms_opt(0, 0, 0).ok_or_else(|| {
        StoreError::InvalidArgument(format!("log index date {date:?} has no midnight"))
    })?;
    Ok(midnight.and_utc().timestamp())
}

/// Records a log file on disk, replacing an earlier record of the same file.
///
/// A rolling log file is re-indexed whenever it grows, so the unique key is
/// the file itself and the counts are overwritten rather than appended.
pub fn index_log_file(conn: &Connection, entry: &LogIndexEntry) -> Result<i64, StoreError> {
    // Midnight of the log's own date, so indexing a file cannot drag a host's
    // first_seen back to the epoch.
    touch_host(conn, entry.host_id, date_to_ts(&entry.date)?)?;
    let entry_id: i64 = conn.query_row(
        "INSERT INTO log_index (host_id, container_id, date, path, line_count, byte_size) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6) \
         ON CONFLICT(host_id, container_id, date, path) DO UPDATE SET \
             line_count = excluded.line_count, byte_size = excluded.byte_size \
         RETURNING entry_id",
        params![
            entry.host_id,
            entry.container_id,
            entry.date,
            entry.path,
            entry.line_count,
            entry.byte_size,
        ],
        |row| row.get(0),
    )?;
    Ok(entry_id)
}

/// Log files for a container between two `YYYY-MM-DD` dates, both inclusive.
///
/// Dates are compared as text, which is chronological for this format. A
/// search over a date range reads this list and opens only the files it names.
pub fn log_files_between(
    conn: &Connection,
    host_id: i64,
    container_id: &str,
    from_date: &str,
    to_date: &str,
) -> Result<Vec<LogIndexEntry>, StoreError> {
    let mut stmt = conn.prepare_cached(
        "SELECT entry_id, host_id, container_id, date, path, line_count, byte_size \
         FROM log_index \
         WHERE host_id = ?1 AND container_id = ?2 AND date >= ?3 AND date <= ?4 \
         ORDER BY date, path",
    )?;
    let rows = stmt.query_map(params![host_id, container_id, from_date, to_date], |row| {
        Ok(LogIndexEntry {
            entry_id: row.get(0)?,
            host_id: row.get(1)?,
            container_id: row.get(2)?,
            date: row.get(3)?,
            path: row.get(4)?,
            line_count: row.get(5)?,
            byte_size: row.get(6)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Opens a session row and returns its id.
pub fn start_session(conn: &Connection, started: i64) -> Result<i64, StoreError> {
    conn.execute(
        "INSERT INTO sessions (started, ended, hosts_touched) VALUES (?1, NULL, '[]')",
        params![started],
    )?;
    Ok(conn.last_insert_rowid())
}

/// Closes a session and records which hosts it touched.
///
/// A session left open means the process did not exit cleanly, which is worth
/// being able to see later, so `ended` is only written here.
pub fn end_session(
    conn: &Connection,
    id: i64,
    ended: i64,
    hosts_touched: &[i64],
) -> Result<(), StoreError> {
    let encoded = serde_json::to_string(hosts_touched).map_err(|e| StoreError::Decode {
        field: "sessions.hosts_touched",
        detail: e.to_string(),
    })?;
    conn.execute(
        "UPDATE sessions SET ended = ?2, hosts_touched = ?3 WHERE id = ?1",
        params![id, ended, encoded],
    )?;
    Ok(())
}

/// The most recent sessions, newest first.
pub fn recent_sessions(conn: &Connection, limit: usize) -> Result<Vec<SessionRecord>, StoreError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, started, ended, hosts_touched FROM sessions ORDER BY started DESC, id DESC \
         LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], |row| {
        let encoded: String = row.get(3)?;
        Ok((
            row.get::<_, i64>(0)?,
            row.get::<_, i64>(1)?,
            row.get::<_, Option<i64>>(2)?,
            encoded,
        ))
    })?;
    let mut out = Vec::new();
    for row in rows {
        let (id, started, ended, encoded) = row?;
        let hosts_touched: Vec<i64> =
            serde_json::from_str(&encoded).map_err(|e| StoreError::Decode {
                field: "sessions.hosts_touched",
                detail: e.to_string(),
            })?;
        out.push(SessionRecord {
            id,
            started,
            ended,
            hosts_touched,
        });
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::store::schema;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        schema::configure(&conn).unwrap();
        schema::ensure_schema(&conn).unwrap();
        conn
    }

    fn container_event(host_id: i64, ts: i64, event: &str) -> ContainerEvent {
        ContainerEvent {
            event_id: None,
            host_id,
            container_id: "abc123".to_string(),
            container_name: Some("web".to_string()),
            ts,
            event: event.to_string(),
            detail: None,
        }
    }

    #[test]
    fn container_events_round_trip_in_time_order() {
        let conn = conn();
        record_container_event(&conn, &container_event(1, 30, "die")).unwrap();
        record_container_event(&conn, &container_event(1, 10, "start")).unwrap();
        let events = container_events_between(&conn, 1, 0, 100).unwrap();
        assert_eq!(events.len(), 2);
        assert_eq!(events[0].event, "start");
        assert_eq!(events[1].event, "die");
        assert!(events[0].event_id.is_some());
        assert_eq!(events[0].container_name.as_deref(), Some("web"));
    }

    #[test]
    fn container_events_respect_the_window_and_the_host() {
        let conn = conn();
        record_container_event(&conn, &container_event(1, 10, "start")).unwrap();
        record_container_event(&conn, &container_event(2, 10, "start")).unwrap();
        assert_eq!(container_events_between(&conn, 1, 10, 10).unwrap().len(), 1);
        assert!(
            container_events_between(&conn, 1, 11, 20)
                .unwrap()
                .is_empty()
        );
        assert!(
            container_events_between(&conn, 3, 0, 100)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn container_event_registers_an_unseen_host() {
        let conn = conn();
        record_container_event(&conn, &container_event(9, 55, "oom")).unwrap();
        let hosts = crate::store::metrics::hosts(&conn).unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].host_id, 9);
    }

    #[test]
    fn k8s_events_filter_by_namespace_when_asked() {
        let conn = conn();
        for (ns, name) in [("default", "web-0"), ("kube-system", "coredns-1")] {
            record_k8s_event(
                &conn,
                &K8sEvent {
                    event_id: None,
                    context: "prod".to_string(),
                    namespace: ns.to_string(),
                    kind: "Pod".to_string(),
                    name: name.to_string(),
                    ts: 100,
                    reason: Some("BackOff".to_string()),
                    message: Some("restarting".to_string()),
                },
            )
            .unwrap();
        }
        assert_eq!(
            k8s_events_between(&conn, "prod", None, 0, 200)
                .unwrap()
                .len(),
            2
        );
        let filtered = k8s_events_between(&conn, "prod", Some("default"), 0, 200).unwrap();
        assert_eq!(filtered.len(), 1);
        assert_eq!(filtered[0].name, "web-0");
        assert!(
            k8s_events_between(&conn, "staging", None, 0, 200)
                .unwrap()
                .is_empty()
        );
    }

    fn log_entry(date: &str, lines: i64) -> LogIndexEntry {
        LogIndexEntry {
            entry_id: None,
            host_id: 1,
            container_id: "abc123".to_string(),
            date: date.to_string(),
            path: format!("/var/log/abc123-{date}.log"),
            line_count: lines,
            byte_size: lines * 80,
        }
    }

    #[test]
    fn log_index_lookup_narrows_to_a_date_range() {
        let conn = conn();
        for date in ["2026-08-30", "2026-08-31", "2026-09-01", "2026-09-02"] {
            index_log_file(&conn, &log_entry(date, 100)).unwrap();
        }
        let found = log_files_between(&conn, 1, "abc123", "2026-08-31", "2026-09-01").unwrap();
        assert_eq!(found.len(), 2);
        assert_eq!(found[0].date, "2026-08-31");
        assert_eq!(found[1].date, "2026-09-01");
    }

    #[test]
    fn re_indexing_the_same_file_updates_the_counts() {
        let conn = conn();
        let first = index_log_file(&conn, &log_entry("2026-09-01", 100)).unwrap();
        let second = index_log_file(&conn, &log_entry("2026-09-01", 250)).unwrap();
        assert_eq!(first, second, "the same file keeps its row");
        let found = log_files_between(&conn, 1, "abc123", "2026-09-01", "2026-09-01").unwrap();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].line_count, 250);
        assert_eq!(found[0].byte_size, 20_000);
    }

    #[test]
    fn a_malformed_index_date_is_rejected() {
        let conn = conn();
        let mut entry = log_entry("2026-09-01", 10);
        entry.date = "01/09/2026".to_string();
        let err = index_log_file(&conn, &entry).unwrap_err();
        assert!(matches!(err, StoreError::InvalidArgument(_)));
    }

    #[test]
    fn indexing_a_log_does_not_backdate_the_host() {
        let conn = conn();
        index_log_file(&conn, &log_entry("2026-09-01", 10)).unwrap();
        let hosts = crate::store::metrics::hosts(&conn).unwrap();
        assert_eq!(hosts.len(), 1);
        assert!(
            hosts[0].first_seen > 1_700_000_000,
            "first_seen should be the log date, got {}",
            hosts[0].first_seen
        );
    }

    #[test]
    fn sessions_open_and_close() {
        let conn = conn();
        let id = start_session(&conn, 1_000).unwrap();
        let open = recent_sessions(&conn, 10).unwrap();
        assert_eq!(open.len(), 1);
        assert_eq!(open[0].ended, None);
        assert!(open[0].hosts_touched.is_empty());

        end_session(&conn, id, 2_000, &[1, 5, 9]).unwrap();
        let closed = recent_sessions(&conn, 10).unwrap();
        assert_eq!(closed[0].ended, Some(2_000));
        assert_eq!(closed[0].hosts_touched, vec![1, 5, 9]);
    }

    #[test]
    fn recent_sessions_are_newest_first_and_respect_the_limit() {
        let conn = conn();
        for started in [100, 200, 300] {
            start_session(&conn, started).unwrap();
        }
        let sessions = recent_sessions(&conn, 2).unwrap();
        assert_eq!(sessions.len(), 2);
        assert_eq!(sessions[0].started, 300);
        assert_eq!(sessions[1].started, 200);
    }

    #[test]
    fn ending_an_unknown_session_changes_nothing() {
        let conn = conn();
        end_session(&conn, 42, 1, &[]).unwrap();
        assert!(recent_sessions(&conn, 10).unwrap().is_empty());
    }
}
