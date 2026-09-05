//! Persistence for alerts: opening a row when a rule starts breaching,
//! closing it when the rule stops, and reading rows back.
//!
//! Split from [`super::alerts`] so rule evaluation stays a pure function with
//! no database in sight.

use std::collections::HashSet;

use rusqlite::{Connection, params};
use serde::{Deserialize, Serialize};

use super::alerts::{AlertFiring, AlertRule, evaluate};
use super::error::StoreError;
use super::metrics::touch_host;
use super::types::MetricSample;

/// A stored alert row.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct AlertRecord {
    /// Row id.
    pub id: i64,
    /// Host the alert is about.
    pub host_id: i64,
    /// When the alert opened.
    pub ts: i64,
    /// [`AlertRule::key`] of the rule that opened it.
    pub rule: String,
    /// Value at the moment it opened.
    pub value: f64,
    /// Threshold that was breached.
    pub threshold: f64,
    /// When it closed, or `None` while it is still firing.
    pub cleared_ts: Option<i64>,
}

/// What one ingest did to the alert table.
#[derive(Debug, Clone, Default, PartialEq)]
pub struct AlertUpdate {
    /// Rules that opened a new alert on this sample.
    pub fired: Vec<AlertFiring>,
    /// Rule keys whose open alert was closed by this sample.
    pub cleared: Vec<String>,
}

impl AlertUpdate {
    /// True when the sample changed nothing.
    pub fn is_empty(&self) -> bool {
        self.fired.is_empty() && self.cleared.is_empty()
    }
}

/// Opens alerts for newly breaching rules and closes alerts that stopped
/// breaching.
///
/// Recording one row per breaching sample would turn a host that sits at 95%
/// CPU for an hour into hundreds of rows saying the same thing, so a rule that
/// is already firing for a host is left alone until it clears.
///
/// A rule whose metric is missing from this sample leaves any open alert open:
/// a host that stops reporting its temperature has not cooled down.
pub fn apply(
    conn: &Connection,
    sample: &MetricSample,
    rules: &[AlertRule],
) -> Result<AlertUpdate, StoreError> {
    if rules.is_empty() {
        return Ok(AlertUpdate::default());
    }
    touch_host(conn, sample.host_id, sample.ts)?;

    let firings = evaluate(sample, rules);
    let firing_keys: HashSet<String> = firings.iter().map(|f| f.rule.key()).collect();

    let mut update = AlertUpdate::default();
    for firing in firings {
        let key = firing.rule.key();
        if is_active(conn, firing.host_id, &key)? {
            continue;
        }
        conn.execute(
            "INSERT INTO alerts (host_id, ts, rule, value, threshold, cleared_ts) \
             VALUES (?1, ?2, ?3, ?4, ?5, NULL)",
            params![
                firing.host_id,
                firing.ts,
                key,
                firing.value,
                firing.rule.threshold
            ],
        )?;
        update.fired.push(firing);
    }

    for rule in rules {
        let key = rule.key();
        if firing_keys.contains(&key) {
            continue;
        }
        if rule.metric.kind().value_of(sample).is_none() {
            continue;
        }
        let closed = conn.execute(
            "UPDATE alerts SET cleared_ts = ?3 \
             WHERE host_id = ?1 AND rule = ?2 AND cleared_ts IS NULL",
            params![sample.host_id, key, sample.ts],
        )?;
        if closed > 0 {
            update.cleared.push(key);
        }
    }
    Ok(update)
}

/// Whether a rule already has an open alert for a host.
fn is_active(conn: &Connection, host_id: i64, rule_key: &str) -> Result<bool, StoreError> {
    let count: i64 = conn.query_row(
        "SELECT COUNT(*) FROM alerts \
         WHERE host_id = ?1 AND rule = ?2 AND cleared_ts IS NULL",
        params![host_id, rule_key],
        |row| row.get(0),
    )?;
    Ok(count > 0)
}

fn map_alert(row: &rusqlite::Row<'_>) -> rusqlite::Result<AlertRecord> {
    Ok(AlertRecord {
        id: row.get(0)?,
        host_id: row.get(1)?,
        ts: row.get(2)?,
        rule: row.get(3)?,
        value: row.get(4)?,
        threshold: row.get(5)?,
        cleared_ts: row.get(6)?,
    })
}

/// Alerts still firing for one host, newest first.
pub fn active_alerts(conn: &Connection, host_id: i64) -> Result<Vec<AlertRecord>, StoreError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, host_id, ts, rule, value, threshold, cleared_ts FROM alerts \
         WHERE host_id = ?1 AND cleared_ts IS NULL ORDER BY ts DESC, id DESC",
    )?;
    let rows = stmt.query_map(params![host_id], map_alert)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

/// The most recently opened alerts across every host, firing or cleared.
pub fn recent_alerts(conn: &Connection, limit: usize) -> Result<Vec<AlertRecord>, StoreError> {
    let mut stmt = conn.prepare_cached(
        "SELECT id, host_id, ts, rule, value, threshold, cleared_ts FROM alerts \
         ORDER BY ts DESC, id DESC LIMIT ?1",
    )?;
    let rows = stmt.query_map(params![limit as i64], map_alert)?;
    let mut out = Vec::new();
    for row in rows {
        out.push(row?);
    }
    Ok(out)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::store::alerts::{AlertMetric, Comparison};
    use crate::store::metrics::insert_sample;
    use crate::store::schema;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        schema::configure(&conn).unwrap();
        schema::ensure_schema(&conn).unwrap();
        conn
    }

    fn cpu_rule() -> AlertRule {
        AlertRule::new(AlertMetric::CpuPercent, Comparison::Above, 90.0)
    }

    fn cpu_sample(ts: i64, value: f64) -> MetricSample {
        let mut sample = MetricSample::new(1, ts);
        sample.cpu_percent = Some(value);
        sample
    }

    #[test]
    fn an_alert_fires_once_and_does_not_re_fire_while_active() {
        let conn = conn();
        let rules = [cpu_rule()];

        let first = apply(&conn, &cpu_sample(10, 95.0), &rules).unwrap();
        assert_eq!(first.fired.len(), 1);
        assert!(first.cleared.is_empty());

        let second = apply(&conn, &cpu_sample(20, 97.0), &rules).unwrap();
        assert!(second.is_empty(), "still firing, nothing to record");

        assert_eq!(recent_alerts(&conn, 10).unwrap().len(), 1);
        let active = active_alerts(&conn, 1).unwrap();
        assert_eq!(active.len(), 1);
        assert_eq!(active[0].ts, 10, "the alert keeps its opening time");
        assert_eq!(active[0].value, 95.0);
        assert_eq!(active[0].threshold, 90.0);
    }

    #[test]
    fn an_alert_clears_when_the_value_comes_back() {
        let conn = conn();
        let rules = [cpu_rule()];
        apply(&conn, &cpu_sample(10, 95.0), &rules).unwrap();

        let update = apply(&conn, &cpu_sample(30, 40.0), &rules).unwrap();
        assert_eq!(update.cleared, vec!["cpu_percent:above:90".to_string()]);
        assert!(update.fired.is_empty());

        assert!(active_alerts(&conn, 1).unwrap().is_empty());
        let all = recent_alerts(&conn, 10).unwrap();
        assert_eq!(all.len(), 1);
        assert_eq!(all[0].cleared_ts, Some(30));
    }

    #[test]
    fn clearing_twice_reports_nothing_the_second_time() {
        let conn = conn();
        let rules = [cpu_rule()];
        apply(&conn, &cpu_sample(10, 95.0), &rules).unwrap();
        apply(&conn, &cpu_sample(30, 40.0), &rules).unwrap();
        let again = apply(&conn, &cpu_sample(40, 41.0), &rules).unwrap();
        assert!(again.is_empty());
    }

    #[test]
    fn a_rule_can_fire_again_after_it_cleared() {
        let conn = conn();
        let rules = [cpu_rule()];
        apply(&conn, &cpu_sample(10, 95.0), &rules).unwrap();
        apply(&conn, &cpu_sample(20, 40.0), &rules).unwrap();
        let refired = apply(&conn, &cpu_sample(30, 99.0), &rules).unwrap();
        assert_eq!(refired.fired.len(), 1);

        let all = recent_alerts(&conn, 10).unwrap();
        assert_eq!(all.len(), 2, "a second episode is a second row");
        assert_eq!(all[0].ts, 30);
        assert_eq!(all[0].cleared_ts, None);
        assert_eq!(all[1].cleared_ts, Some(20));
    }

    #[test]
    fn a_missing_reading_does_not_clear_an_open_alert() {
        let conn = conn();
        let rules = [AlertRule::new(
            AlertMetric::TemperatureC,
            Comparison::Above,
            70.0,
        )];
        let mut hot = MetricSample::new(1, 10);
        hot.temperature_c = Some(85.0);
        assert_eq!(apply(&conn, &hot, &rules).unwrap().fired.len(), 1);

        // The next poll reports no temperature at all.
        let silent = MetricSample::new(1, 20);
        let update = apply(&conn, &silent, &rules).unwrap();
        assert!(update.is_empty());
        assert_eq!(active_alerts(&conn, 1).unwrap().len(), 1);
    }

    #[test]
    fn below_rules_fire_and_clear() {
        let conn = conn();
        let rules = [AlertRule::new(
            AlertMetric::DiskUsedPercent,
            Comparison::Below,
            10.0,
        )];
        let mut low = MetricSample::new(1, 10);
        low.disk_used_bytes = Some(50);
        low.disk_total_bytes = Some(1000);
        assert_eq!(apply(&conn, &low, &rules).unwrap().fired.len(), 1);

        let mut normal = MetricSample::new(1, 20);
        normal.disk_used_bytes = Some(500);
        normal.disk_total_bytes = Some(1000);
        assert_eq!(apply(&conn, &normal, &rules).unwrap().cleared.len(), 1);
    }

    #[test]
    fn alerts_are_tracked_per_host() {
        let conn = conn();
        let rules = [cpu_rule()];
        apply(&conn, &cpu_sample(10, 95.0), &rules).unwrap();
        let mut other = cpu_sample(10, 96.0);
        other.host_id = 2;
        apply(&conn, &other, &rules).unwrap();

        assert_eq!(active_alerts(&conn, 1).unwrap().len(), 1);
        assert_eq!(active_alerts(&conn, 2).unwrap().len(), 1);
        assert_eq!(recent_alerts(&conn, 10).unwrap().len(), 2);
    }

    #[test]
    fn no_rules_means_no_work() {
        let conn = conn();
        assert!(apply(&conn, &cpu_sample(10, 99.0), &[]).unwrap().is_empty());
        assert!(recent_alerts(&conn, 10).unwrap().is_empty());
    }

    #[test]
    fn recent_alerts_are_newest_first_and_respect_the_limit() {
        let conn = conn();
        let rules = [
            AlertRule::new(AlertMetric::CpuPercent, Comparison::Above, 10.0),
            AlertRule::new(AlertMetric::CpuPercent, Comparison::Above, 20.0),
            AlertRule::new(AlertMetric::CpuPercent, Comparison::Above, 30.0),
        ];
        insert_sample(&conn, &cpu_sample(100, 99.0)).unwrap();
        apply(&conn, &cpu_sample(100, 99.0), &rules).unwrap();

        let limited = recent_alerts(&conn, 2).unwrap();
        assert_eq!(limited.len(), 2);
        assert_eq!(recent_alerts(&conn, 10).unwrap().len(), 3);
    }
}
