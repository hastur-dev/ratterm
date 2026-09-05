//! Sample ingest and the row-level sample queries.
//!
//! Everything here takes a `&Connection` so the same code runs inside a
//! transaction (batch ingest) and outside one (a single sample), and so the
//! public [`super::MetricStore`] stays a thin wrapper.

use rusqlite::{Connection, Row, params};

use super::error::StoreError;
use super::types::{HostRow, MetricSample};

/// Column order shared by every read and write of `metric_samples`.
pub const SAMPLE_COLUMNS: &str = "host_id, ts, resolution, cpu_load1, cpu_percent, \
     mem_used_bytes, mem_total_bytes, disk_used_bytes, disk_total_bytes, \
     gpu_util_percent, gpu_mem_used_bytes, temperature_c, uptime_secs";

/// Re-recording the same host, timestamp and resolution replaces the row.
///
/// A collector that retries after a timeout would otherwise fail on the
/// primary key, and two samples for the same second carry no more information
/// than one.
fn insert_sql() -> String {
    format!(
        "INSERT OR REPLACE INTO metric_samples ({SAMPLE_COLUMNS}) \
         VALUES (?1, ?2, ?3, ?4, ?5, ?6, ?7, ?8, ?9, ?10, ?11, ?12, ?13)"
    )
}

fn select_sql(tail: &str) -> String {
    format!("SELECT {SAMPLE_COLUMNS} FROM metric_samples {tail}")
}

/// Builds a [`MetricSample`] from a row selected with [`SAMPLE_COLUMNS`].
pub fn row_to_sample(row: &Row<'_>) -> rusqlite::Result<MetricSample> {
    Ok(MetricSample {
        host_id: row.get(0)?,
        ts: row.get(1)?,
        resolution: row.get(2)?,
        cpu_load1: row.get(3)?,
        cpu_percent: row.get(4)?,
        mem_used_bytes: row.get(5)?,
        mem_total_bytes: row.get(6)?,
        disk_used_bytes: row.get(7)?,
        disk_total_bytes: row.get(8)?,
        gpu_util_percent: row.get(9)?,
        gpu_mem_used_bytes: row.get(10)?,
        temperature_c: row.get(11)?,
        uptime_secs: row.get(12)?,
    })
}

/// Makes sure `hosts` has a row for `host_id` and widens its seen window.
///
/// Ingest calls this first because `metric_samples.host_id` is a foreign key.
/// The label is left alone: a host discovered by the push daemon has an id
/// before anything knows its name, and a later [`set_host_label`] fills it in.
pub fn touch_host(conn: &Connection, host_id: i64, ts: i64) -> Result<(), StoreError> {
    conn.execute(
        "INSERT INTO hosts (host_id, label, first_seen, last_seen) \
         VALUES (?1, NULL, ?2, ?2) \
         ON CONFLICT(host_id) DO UPDATE SET \
             first_seen = MIN(hosts.first_seen, excluded.first_seen), \
             last_seen  = MAX(hosts.last_seen,  excluded.last_seen)",
        params![host_id, ts],
    )?;
    Ok(())
}

/// Returns the id for `label`, allocating one on first sight.
///
/// The rest of the application identifies hosts by name; the store identifies
/// them by integer so the time-series tables stay narrow. This is the bridge.
pub fn register_host(conn: &Connection, label: &str, now: i64) -> Result<i64, StoreError> {
    let host_id: i64 = conn.query_row(
        "INSERT INTO hosts (label, first_seen, last_seen) VALUES (?1, ?2, ?2) \
         ON CONFLICT(label) DO UPDATE SET last_seen = MAX(hosts.last_seen, excluded.last_seen) \
         RETURNING host_id",
        params![label, now],
        |row| row.get(0),
    )?;
    Ok(host_id)
}

/// Attaches or replaces the human-facing name of a host.
pub fn set_host_label(conn: &Connection, host_id: i64, label: &str) -> Result<(), StoreError> {
    conn.execute(
        "UPDATE hosts SET label = ?2 WHERE host_id = ?1",
        params![host_id, label],
    )?;
    Ok(())
}

/// Every known host, oldest first seen first.
pub fn hosts(conn: &Connection) -> Result<Vec<HostRow>, StoreError> {
    let mut stmt = conn.prepare(
        "SELECT host_id, label, first_seen, last_seen FROM hosts ORDER BY first_seen, host_id",
    )?;
    let rows = stmt.query_map([], |row| {
        Ok(HostRow {
            host_id: row.get(0)?,
            label: row.get(1)?,
            first_seen: row.get(2)?,
            last_seen: row.get(3)?,
        })
    })?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Writes one sample, registering the host if it is new.
pub fn insert_sample(conn: &Connection, sample: &MetricSample) -> Result<(), StoreError> {
    touch_host(conn, sample.host_id, sample.ts)?;
    let mut stmt = conn.prepare_cached(&insert_sql())?;
    stmt.execute(params![
        sample.host_id,
        sample.ts,
        sample.resolution,
        sample.cpu_load1,
        sample.cpu_percent,
        sample.mem_used_bytes,
        sample.mem_total_bytes,
        sample.disk_used_bytes,
        sample.disk_total_bytes,
        sample.gpu_util_percent,
        sample.gpu_mem_used_bytes,
        sample.temperature_c,
        sample.uptime_secs,
    ])?;
    Ok(())
}

/// Writes many samples through one prepared statement.
///
/// The caller is expected to have opened a transaction; without one SQLite
/// commits per statement and a few hundred samples take seconds instead of
/// milliseconds.
pub fn insert_samples(conn: &Connection, samples: &[MetricSample]) -> Result<usize, StoreError> {
    if samples.is_empty() {
        return Ok(0);
    }
    for sample in samples {
        touch_host(conn, sample.host_id, sample.ts)?;
    }
    let mut stmt = conn.prepare_cached(&insert_sql())?;
    for sample in samples {
        stmt.execute(params![
            sample.host_id,
            sample.ts,
            sample.resolution,
            sample.cpu_load1,
            sample.cpu_percent,
            sample.mem_used_bytes,
            sample.mem_total_bytes,
            sample.disk_used_bytes,
            sample.disk_total_bytes,
            sample.gpu_util_percent,
            sample.gpu_mem_used_bytes,
            sample.temperature_c,
            sample.uptime_secs,
        ])?;
    }
    Ok(samples.len())
}

/// Samples for one host in `[from_ts, to_ts]`, both ends inclusive.
///
/// A reversed range yields nothing rather than an error: callers derive the
/// bounds from a scrolling viewport and an empty result is the useful answer.
pub fn samples_between(
    conn: &Connection,
    host_id: i64,
    from_ts: i64,
    to_ts: i64,
) -> Result<Vec<MetricSample>, StoreError> {
    let sql = select_sql("WHERE host_id = ?1 AND ts >= ?2 AND ts <= ?3 ORDER BY ts, resolution");
    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(params![host_id, from_ts, to_ts], row_to_sample)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// Most recent sample for a host, whatever its resolution.
pub fn latest_sample(conn: &Connection, host_id: i64) -> Result<Option<MetricSample>, StoreError> {
    let sql = select_sql("WHERE host_id = ?1 ORDER BY ts DESC LIMIT 1");
    let mut stmt = conn.prepare_cached(&sql)?;
    let mut rows = stmt.query_map(params![host_id], row_to_sample)?;
    match rows.next() {
        Some(row) => Ok(Some(row?)),
        None => Ok(None),
    }
}

/// Timestamp of the last sample from a host, or `None` if it has never
/// reported.
///
/// The caller compares this against the current time to decide whether the
/// host counts as offline; the store does not read the clock so the decision
/// stays testable and so a stale clock cannot mark a live host down.
pub fn offline_since(conn: &Connection, host_id: i64) -> Result<Option<i64>, StoreError> {
    let ts: Option<i64> = conn.query_row(
        "SELECT MAX(ts) FROM metric_samples WHERE host_id = ?1",
        params![host_id],
        |row| row.get(0),
    )?;
    Ok(ts)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::store::schema;
    use crate::store::types::RESOLUTION_MINUTE;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        schema::configure(&conn).unwrap();
        schema::ensure_schema(&conn).unwrap();
        conn
    }

    fn full_sample(host_id: i64, ts: i64) -> MetricSample {
        MetricSample {
            host_id,
            ts,
            resolution: 0,
            cpu_load1: Some(1.25),
            cpu_percent: Some(37.5),
            mem_used_bytes: Some(2_000_000_000),
            mem_total_bytes: Some(8_000_000_000),
            disk_used_bytes: Some(50_000_000_000),
            disk_total_bytes: Some(500_000_000_000),
            gpu_util_percent: Some(88.0),
            gpu_mem_used_bytes: Some(4_000_000_000),
            temperature_c: Some(61.5),
            uptime_secs: Some(86_400),
        }
    }

    #[test]
    fn sample_with_every_metric_round_trips() {
        let conn = conn();
        let sample = full_sample(1, 1_700_000_000);
        insert_sample(&conn, &sample).unwrap();
        let read = latest_sample(&conn, 1).unwrap().unwrap();
        assert_eq!(read, sample);
    }

    #[test]
    fn sample_with_no_optional_metrics_round_trips() {
        let conn = conn();
        let sample = MetricSample::new(2, 500);
        insert_sample(&conn, &sample).unwrap();
        let read = latest_sample(&conn, 2).unwrap().unwrap();
        assert_eq!(read, sample);
        assert!(read.cpu_load1.is_none());
        assert!(read.uptime_secs.is_none());
    }

    #[test]
    fn ingest_registers_the_host_and_widens_the_seen_window() {
        let conn = conn();
        insert_sample(&conn, &MetricSample::new(7, 200)).unwrap();
        insert_sample(&conn, &MetricSample::new(7, 100)).unwrap();
        insert_sample(&conn, &MetricSample::new(7, 300)).unwrap();
        let hosts = hosts(&conn).unwrap();
        assert_eq!(hosts.len(), 1);
        assert_eq!(hosts[0].host_id, 7);
        assert_eq!(hosts[0].first_seen, 100);
        assert_eq!(hosts[0].last_seen, 300);
        assert_eq!(hosts[0].label, None);
    }

    #[test]
    fn register_host_is_stable_for_the_same_label() {
        let conn = conn();
        let a = register_host(&conn, "cthulhu-computer", 10).unwrap();
        let b = register_host(&conn, "cthulhu-computer", 20).unwrap();
        let other = register_host(&conn, "rock-5c", 20).unwrap();
        assert_eq!(a, b);
        assert_ne!(a, other);
    }

    #[test]
    fn set_host_label_names_a_host_discovered_by_id() {
        let conn = conn();
        insert_sample(&conn, &MetricSample::new(4, 1)).unwrap();
        set_host_label(&conn, 4, "gpu-box").unwrap();
        let hosts = hosts(&conn).unwrap();
        assert_eq!(hosts[0].label.as_deref(), Some("gpu-box"));
    }

    #[test]
    fn re_recording_the_same_second_replaces_rather_than_fails() {
        let conn = conn();
        let mut sample = MetricSample::new(1, 42);
        sample.cpu_percent = Some(10.0);
        insert_sample(&conn, &sample).unwrap();
        sample.cpu_percent = Some(90.0);
        insert_sample(&conn, &sample).unwrap();
        let rows = samples_between(&conn, 1, 0, 100).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cpu_percent, Some(90.0));
    }

    #[test]
    fn a_minute_row_and_a_raw_row_can_share_a_timestamp() {
        let conn = conn();
        let raw = MetricSample::new(1, 60);
        let mut minute = MetricSample::new(1, 60);
        minute.resolution = RESOLUTION_MINUTE;
        insert_sample(&conn, &raw).unwrap();
        insert_sample(&conn, &minute).unwrap();
        assert_eq!(samples_between(&conn, 1, 60, 60).unwrap().len(), 2);
    }

    #[test]
    fn samples_between_includes_both_ends() {
        let conn = conn();
        for ts in [10, 20, 30, 40] {
            insert_sample(&conn, &MetricSample::new(1, ts)).unwrap();
        }
        let inclusive = samples_between(&conn, 1, 20, 30).unwrap();
        assert_eq!(
            inclusive.iter().map(|s| s.ts).collect::<Vec<_>>(),
            vec![20, 30]
        );

        let outside = samples_between(&conn, 1, 21, 29).unwrap();
        assert!(outside.is_empty(), "range between samples must be empty");
    }

    #[test]
    fn samples_between_handles_empty_and_reversed_ranges() {
        let conn = conn();
        insert_sample(&conn, &MetricSample::new(1, 100)).unwrap();
        assert!(samples_between(&conn, 1, 200, 300).unwrap().is_empty());
        assert!(
            samples_between(&conn, 1, 300, 200).unwrap().is_empty(),
            "a reversed range yields nothing"
        );
        assert_eq!(samples_between(&conn, 1, 100, 100).unwrap().len(), 1);
    }

    #[test]
    fn samples_between_is_scoped_to_one_host() {
        let conn = conn();
        insert_sample(&conn, &MetricSample::new(1, 10)).unwrap();
        insert_sample(&conn, &MetricSample::new(2, 10)).unwrap();
        assert_eq!(samples_between(&conn, 1, 0, 100).unwrap().len(), 1);
    }

    #[test]
    fn latest_sample_and_offline_since_agree() {
        let conn = conn();
        for ts in [5, 900, 400] {
            insert_sample(&conn, &MetricSample::new(1, ts)).unwrap();
        }
        assert_eq!(latest_sample(&conn, 1).unwrap().unwrap().ts, 900);
        assert_eq!(offline_since(&conn, 1).unwrap(), Some(900));
    }

    #[test]
    fn offline_since_is_none_without_any_samples() {
        let conn = conn();
        assert_eq!(offline_since(&conn, 1).unwrap(), None);
        assert!(latest_sample(&conn, 1).unwrap().is_none());

        // A host that exists but has never reported is also None.
        touch_host(&conn, 1, 50).unwrap();
        assert_eq!(offline_since(&conn, 1).unwrap(), None);
    }

    #[test]
    fn batch_insert_of_several_hundred_samples() {
        let mut conn = conn();
        let samples: Vec<MetricSample> = (0..500)
            .map(|i| {
                let mut s = MetricSample::new(1 + i % 3, 1_000 + i);
                s.cpu_percent = Some(i as f64 / 10.0);
                s
            })
            .collect();

        let tx = conn.transaction().unwrap();
        let written = insert_samples(&tx, &samples).unwrap();
        tx.commit().unwrap();

        assert_eq!(written, 500);
        let total: i64 = conn
            .query_row("SELECT COUNT(*) FROM metric_samples", [], |row| row.get(0))
            .unwrap();
        assert_eq!(total, 500);
        let host_count: i64 = conn
            .query_row("SELECT COUNT(*) FROM hosts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(host_count, 3);
    }

    #[test]
    fn batch_insert_of_nothing_writes_nothing() {
        let conn = conn();
        assert_eq!(insert_samples(&conn, &[]).unwrap(), 0);
    }
}
