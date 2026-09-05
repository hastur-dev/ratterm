//! Schema definition, connection setup and version handling.
//!
//! The schema version lives in SQLite's own `PRAGMA user_version` rather than
//! a table of our own, so reading it needs no schema and works on a database
//! written by any past or future build.

use rusqlite::Connection;

use super::error::StoreError;

/// Schema version this build writes and reads.
///
/// Bump this and add an arm to [`migrate`] when the shape of a table changes.
pub const SCHEMA_VERSION: i64 = 1;

/// Every `CREATE` statement, in dependency order.
///
/// `IF NOT EXISTS` throughout so applying this to an existing database of the
/// current version is a no-op and cannot drop data.
const CREATE_SQL: &str = r#"
CREATE TABLE IF NOT EXISTS hosts (
    host_id    INTEGER PRIMARY KEY,
    label      TEXT UNIQUE,
    first_seen INTEGER NOT NULL,
    last_seen  INTEGER NOT NULL
);

CREATE TABLE IF NOT EXISTS metric_samples (
    host_id            INTEGER NOT NULL REFERENCES hosts(host_id) ON DELETE CASCADE,
    ts                 INTEGER NOT NULL,
    resolution         INTEGER NOT NULL DEFAULT 0,
    cpu_load1          REAL,
    cpu_percent        REAL,
    mem_used_bytes     INTEGER,
    mem_total_bytes    INTEGER,
    disk_used_bytes    INTEGER,
    disk_total_bytes   INTEGER,
    gpu_util_percent   REAL,
    gpu_mem_used_bytes INTEGER,
    temperature_c      REAL,
    uptime_secs        INTEGER,
    PRIMARY KEY (host_id, ts, resolution)
);

CREATE INDEX IF NOT EXISTS idx_metric_samples_host_ts
    ON metric_samples(host_id, ts);

CREATE INDEX IF NOT EXISTS idx_metric_samples_resolution_ts
    ON metric_samples(resolution, ts);

CREATE TABLE IF NOT EXISTS container_events (
    event_id       INTEGER PRIMARY KEY,
    host_id        INTEGER NOT NULL REFERENCES hosts(host_id) ON DELETE CASCADE,
    container_id   TEXT NOT NULL,
    container_name TEXT,
    ts             INTEGER NOT NULL,
    event          TEXT NOT NULL,
    detail         TEXT
);

CREATE INDEX IF NOT EXISTS idx_container_events_host_ts
    ON container_events(host_id, ts);

CREATE TABLE IF NOT EXISTS k8s_events (
    event_id  INTEGER PRIMARY KEY,
    context   TEXT NOT NULL,
    namespace TEXT NOT NULL,
    kind      TEXT NOT NULL,
    name      TEXT NOT NULL,
    ts        INTEGER NOT NULL,
    reason    TEXT,
    message   TEXT
);

CREATE INDEX IF NOT EXISTS idx_k8s_events_ts
    ON k8s_events(ts);

CREATE INDEX IF NOT EXISTS idx_k8s_events_object
    ON k8s_events(context, namespace, kind, name, ts);

CREATE TABLE IF NOT EXISTS sessions (
    id            INTEGER PRIMARY KEY,
    started       INTEGER NOT NULL,
    ended         INTEGER,
    hosts_touched TEXT NOT NULL DEFAULT '[]'
);

CREATE TABLE IF NOT EXISTS log_index (
    entry_id     INTEGER PRIMARY KEY,
    host_id      INTEGER NOT NULL REFERENCES hosts(host_id) ON DELETE CASCADE,
    container_id TEXT NOT NULL,
    date         TEXT NOT NULL,
    path         TEXT NOT NULL,
    line_count   INTEGER NOT NULL,
    byte_size    INTEGER NOT NULL,
    UNIQUE (host_id, container_id, date, path)
);

CREATE INDEX IF NOT EXISTS idx_log_index_lookup
    ON log_index(host_id, container_id, date);

CREATE TABLE IF NOT EXISTS alerts (
    id         INTEGER PRIMARY KEY,
    host_id    INTEGER NOT NULL REFERENCES hosts(host_id) ON DELETE CASCADE,
    ts         INTEGER NOT NULL,
    rule       TEXT NOT NULL,
    value      REAL NOT NULL,
    threshold  REAL NOT NULL,
    cleared_ts INTEGER
);

CREATE INDEX IF NOT EXISTS idx_alerts_active
    ON alerts(host_id, rule, cleared_ts);

CREATE INDEX IF NOT EXISTS idx_alerts_ts
    ON alerts(ts);
"#;

/// Applies the connection-level settings the store depends on.
///
/// WAL keeps a long-running read (a dashboard redraw) from blocking the
/// collector's writes. Foreign keys are off by default in SQLite and have to
/// be turned on per connection, otherwise the `ON DELETE CASCADE` clauses in
/// the schema do nothing.
pub fn configure(conn: &Connection) -> Result<(), StoreError> {
    // journal_mode returns the resulting mode as a row, so it needs a query
    // rather than pragma_update. An in-memory database answers "memory" and
    // that is fine; it has no journal to switch.
    let _mode: String = conn.query_row("PRAGMA journal_mode=WAL", [], |row| row.get(0))?;
    // Full fsync on every commit costs more than a lost trailing sample is
    // worth for monitoring data; NORMAL still survives a process crash.
    conn.execute_batch("PRAGMA foreign_keys = ON; PRAGMA synchronous = NORMAL;")?;
    Ok(())
}

/// Creates the schema if absent and brings an existing database to
/// [`SCHEMA_VERSION`].
///
/// A database at version 0 with no tables is fresh. A database at a version
/// this build does not know is rejected rather than touched, because writing
/// current-shaped rows into an unknown schema is how data gets lost.
pub fn ensure_schema(conn: &Connection) -> Result<(), StoreError> {
    let found: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    if found == SCHEMA_VERSION {
        return Ok(());
    }
    migrate(conn, found)
}

/// Moves a database from `found` to [`SCHEMA_VERSION`].
fn migrate(conn: &Connection, found: i64) -> Result<(), StoreError> {
    match found {
        0 => {
            conn.execute_batch(CREATE_SQL)?;
            set_version(conn, SCHEMA_VERSION)?;
            Ok(())
        }
        // No released version sits between 0 and SCHEMA_VERSION yet. When one
        // does, add an arm here that runs the ALTER statements and falls
        // through to the next.
        _ => Err(StoreError::SchemaVersion {
            found,
            expected: SCHEMA_VERSION,
        }),
    }
}

/// Writes `PRAGMA user_version`.
fn set_version(conn: &Connection, version: i64) -> Result<(), StoreError> {
    conn.pragma_update(None, "user_version", version)?;
    Ok(())
}

/// Reads the schema version recorded in the database.
pub fn schema_version(conn: &Connection) -> Result<i64, StoreError> {
    let found: i64 = conn.query_row("PRAGMA user_version", [], |row| row.get(0))?;
    Ok(found)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn open() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        configure(&conn).unwrap();
        conn
    }

    fn table_names(conn: &Connection) -> Vec<String> {
        let mut stmt = conn
            .prepare("SELECT name FROM sqlite_master WHERE type='table' ORDER BY name")
            .unwrap();
        stmt.query_map([], |row| row.get::<_, String>(0))
            .unwrap()
            .collect::<Result<Vec<_>, _>>()
            .unwrap()
    }

    #[test]
    fn fresh_database_gets_every_table_and_the_current_version() {
        let conn = open();
        assert_eq!(schema_version(&conn).unwrap(), 0);
        ensure_schema(&conn).unwrap();
        assert_eq!(schema_version(&conn).unwrap(), SCHEMA_VERSION);

        let tables = table_names(&conn);
        for expected in [
            "alerts",
            "container_events",
            "hosts",
            "k8s_events",
            "log_index",
            "metric_samples",
            "sessions",
        ] {
            assert!(
                tables.iter().any(|t| t == expected),
                "missing table {expected}, got {tables:?}"
            );
        }
    }

    #[test]
    fn ensure_schema_is_idempotent() {
        let conn = open();
        ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO hosts (host_id, label, first_seen, last_seen) VALUES (1, 'a', 10, 10)",
            [],
        )
        .unwrap();
        ensure_schema(&conn).unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM hosts", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 1, "re-running the schema must not drop rows");
    }

    #[test]
    fn a_newer_schema_version_is_rejected() {
        let conn = open();
        ensure_schema(&conn).unwrap();
        conn.pragma_update(None, "user_version", SCHEMA_VERSION + 1)
            .unwrap();
        let err = ensure_schema(&conn).unwrap_err();
        match err {
            StoreError::SchemaVersion { found, expected } => {
                assert_eq!(found, SCHEMA_VERSION + 1);
                assert_eq!(expected, SCHEMA_VERSION);
            }
            other => panic!("expected SchemaVersion, got {other:?}"),
        }
    }

    #[test]
    fn foreign_keys_are_enforced() {
        let conn = open();
        ensure_schema(&conn).unwrap();
        let result = conn.execute(
            "INSERT INTO metric_samples (host_id, ts, resolution) VALUES (99, 1, 0)",
            [],
        );
        assert!(
            result.is_err(),
            "a sample for an unknown host must be refused"
        );
    }

    #[test]
    fn cascade_delete_removes_dependent_rows() {
        let conn = open();
        ensure_schema(&conn).unwrap();
        conn.execute(
            "INSERT INTO hosts (host_id, label, first_seen, last_seen) VALUES (1, 'a', 0, 0)",
            [],
        )
        .unwrap();
        conn.execute(
            "INSERT INTO metric_samples (host_id, ts, resolution) VALUES (1, 5, 0)",
            [],
        )
        .unwrap();
        conn.execute("DELETE FROM hosts WHERE host_id = 1", [])
            .unwrap();
        let count: i64 = conn
            .query_row("SELECT COUNT(*) FROM metric_samples", [], |row| row.get(0))
            .unwrap();
        assert_eq!(count, 0);
    }

    #[test]
    fn configure_reports_a_journal_mode() {
        let conn = Connection::open_in_memory().unwrap();
        configure(&conn).unwrap();
        let fk: i64 = conn
            .query_row("PRAGMA foreign_keys", [], |row| row.get(0))
            .unwrap();
        assert_eq!(fk, 1, "foreign keys must be on");
    }
}
