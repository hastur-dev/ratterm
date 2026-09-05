//! Retention and downsampling.
//!
//! A host polled every five seconds produces about 6.3 million rows a year.
//! Keeping all of them is neither useful nor cheap, so old raw rows are
//! collapsed into one-minute averages, old minute rows into one-hour averages,
//! and anything past the final horizon is deleted.
//!
//! Each row carries its own resolution, so a row that has already been
//! collapsed is never collapsed a second time.

use std::time::Duration;

use rusqlite::{Connection, params};

use super::error::StoreError;
use super::metrics::SAMPLE_COLUMNS;
use super::types::{RESOLUTION_HOUR, RESOLUTION_MINUTE, RESOLUTION_RAW};

/// How long each resolution is kept.
///
/// The three windows are measured from the current time, and each names the
/// age at which rows of that resolution stop being kept at that resolution.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct RetentionPolicy {
    /// Raw rows older than this become one-minute averages.
    pub raw_for: Duration,
    /// One-minute rows older than this become one-hour averages.
    pub minute_for: Duration,
    /// Anything older than this is deleted, whatever its resolution.
    pub hour_for: Duration,
}

impl Default for RetentionPolicy {
    /// A day of raw samples, a month of minute averages, a year of hour
    /// averages. A day of raw data covers "what happened overnight", which is
    /// the question raw resolution is actually needed for.
    fn default() -> Self {
        Self {
            raw_for: Duration::from_secs(24 * 60 * 60),
            minute_for: Duration::from_secs(30 * 24 * 60 * 60),
            hour_for: Duration::from_secs(365 * 24 * 60 * 60),
        }
    }
}

impl RetentionPolicy {
    /// Keeps everything. Useful for a test or a short-lived investigation
    /// where losing resolution would hide the thing being looked for.
    pub fn keep_everything() -> Self {
        let forever = Duration::from_secs(i64::MAX as u64);
        Self {
            raw_for: forever,
            minute_for: forever,
            hour_for: forever,
        }
    }

    /// The three age cutoffs as absolute timestamps, given the current time.
    ///
    /// A window longer than the current time saturates at `i64::MIN`, meaning
    /// "nothing is old enough yet".
    pub fn cutoffs(&self, now_ts: i64) -> Cutoffs {
        Cutoffs {
            raw: now_ts.saturating_sub(as_secs(self.raw_for)),
            minute: now_ts.saturating_sub(as_secs(self.minute_for)),
            hour: now_ts.saturating_sub(as_secs(self.hour_for)),
        }
    }
}

/// Absolute timestamps derived from a [`RetentionPolicy`] and a clock reading.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Cutoffs {
    /// Raw rows strictly older than this are collapsed.
    pub raw: i64,
    /// Minute rows strictly older than this are collapsed.
    pub minute: i64,
    /// Any row strictly older than this is deleted.
    pub hour: i64,
}

fn as_secs(d: Duration) -> i64 {
    i64::try_from(d.as_secs()).unwrap_or(i64::MAX)
}

/// What one [`downsample`] run changed.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
pub struct DownsampleReport {
    /// Raw rows folded into minute averages and removed.
    pub raw_collapsed: u64,
    /// Minute-average rows written by that fold.
    pub minute_written: u64,
    /// Minute rows folded into hour averages and removed.
    pub minute_collapsed: u64,
    /// Hour-average rows written by that fold.
    pub hour_written: u64,
    /// Rows deleted for being past the final horizon.
    pub deleted: u64,
}

impl DownsampleReport {
    /// True when the run left the database unchanged.
    pub fn is_empty(&self) -> bool {
        *self == Self::default()
    }
}

/// Applies `policy` as of `now_ts`.
///
/// The caller supplies the time rather than the function reading the clock, so
/// a test can drive a year of retention in milliseconds and so a wrong system
/// clock cannot delete live data without the caller choosing to.
///
/// The three phases run in order, so a run after a long gap can move a raw row
/// all the way to an hour average, or delete it, in one pass. The whole run is
/// one transaction: a partial downsample would leave rows counted at two
/// resolutions.
pub fn downsample(
    conn: &mut Connection,
    now_ts: i64,
    policy: &RetentionPolicy,
) -> Result<DownsampleReport, StoreError> {
    let cutoffs = policy.cutoffs(now_ts);
    let tx = conn.transaction()?;

    let (raw_collapsed, minute_written) = collapse(
        &tx,
        RESOLUTION_RAW,
        RESOLUTION_MINUTE,
        RESOLUTION_MINUTE,
        cutoffs.raw,
    )?;
    let (minute_collapsed, hour_written) = collapse(
        &tx,
        RESOLUTION_MINUTE,
        RESOLUTION_HOUR,
        RESOLUTION_HOUR,
        cutoffs.minute,
    )?;
    let deleted = tx.execute(
        "DELETE FROM metric_samples WHERE ts < ?1",
        params![cutoffs.hour],
    )?;

    tx.commit()?;
    Ok(DownsampleReport {
        raw_collapsed,
        minute_written,
        minute_collapsed,
        hour_written,
        deleted: deleted as u64,
    })
}

/// Folds rows at `from_resolution` older than `cutoff` into `bucket`-second
/// averages tagged `to_resolution`, and removes the originals.
///
/// The cutoff is first rounded down to a whole bucket. Without that, the
/// bucket straddling the cutoff would be averaged from only the part of it
/// that is old enough, and the next run would overwrite that average with the
/// remainder. Returns `(rows collapsed, rows written)`.
fn collapse(
    conn: &Connection,
    from_resolution: i64,
    to_resolution: i64,
    bucket: i64,
    cutoff: i64,
) -> Result<(u64, u64), StoreError> {
    let aligned = floor_to(cutoff, bucket);

    // Averaging averages is unweighted: an hour row is the mean of its minute
    // rows, not of the raw samples underneath them. Buckets with unequal
    // sample counts therefore skew slightly. Storing per-row counts would fix
    // it and is not worth a column on every sample for monitoring data.
    let sql = format!(
        "INSERT OR REPLACE INTO metric_samples ({SAMPLE_COLUMNS}) \
         SELECT host_id, \
                ts - (((ts % ?2) + ?2) % ?2) AS bucket_ts, \
                ?3, \
                AVG(cpu_load1), \
                AVG(cpu_percent), \
                CAST(ROUND(AVG(mem_used_bytes)) AS INTEGER), \
                CAST(ROUND(AVG(mem_total_bytes)) AS INTEGER), \
                CAST(ROUND(AVG(disk_used_bytes)) AS INTEGER), \
                CAST(ROUND(AVG(disk_total_bytes)) AS INTEGER), \
                AVG(gpu_util_percent), \
                CAST(ROUND(AVG(gpu_mem_used_bytes)) AS INTEGER), \
                AVG(temperature_c), \
                CAST(ROUND(AVG(uptime_secs)) AS INTEGER) \
         FROM metric_samples \
         WHERE resolution = ?1 AND ts < ?4 \
         GROUP BY host_id, bucket_ts"
    );

    let written = conn.execute(
        &sql,
        params![from_resolution, bucket, to_resolution, aligned],
    )?;
    let collapsed = conn.execute(
        "DELETE FROM metric_samples WHERE resolution = ?1 AND ts < ?2",
        params![from_resolution, aligned],
    )?;
    Ok((collapsed as u64, written as u64))
}

/// Rounds `value` down to a multiple of `bucket`, correctly for negative
/// timestamps as well.
fn floor_to(value: i64, bucket: i64) -> i64 {
    if bucket <= 0 {
        return value;
    }
    // Saturating because a policy of "keep everything" produces a cutoff of
    // i64::MIN, and subtracting the remainder from it would overflow.
    value.saturating_sub(value.rem_euclid(bucket))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::store::metrics::{insert_samples, samples_between};
    use crate::store::schema;
    use crate::store::types::MetricSample;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        schema::configure(&conn).unwrap();
        schema::ensure_schema(&conn).unwrap();
        conn
    }

    fn cpu(host_id: i64, ts: i64, value: f64) -> MetricSample {
        let mut sample = MetricSample::new(host_id, ts);
        sample.cpu_percent = Some(value);
        sample.mem_used_bytes = Some((value * 10.0) as i64);
        sample
    }

    fn policy(raw_h: u64, minute_h: u64, hour_h: u64) -> RetentionPolicy {
        RetentionPolicy {
            raw_for: Duration::from_secs(raw_h * 3600),
            minute_for: Duration::from_secs(minute_h * 3600),
            hour_for: Duration::from_secs(hour_h * 3600),
        }
    }

    #[test]
    fn floor_to_handles_both_signs() {
        assert_eq!(floor_to(125, 60), 120);
        assert_eq!(floor_to(120, 60), 120);
        assert_eq!(floor_to(-1, 60), -60);
        assert_eq!(floor_to(5, 0), 5);
    }

    #[test]
    fn default_policy_is_a_day_a_month_and_a_year() {
        let p = RetentionPolicy::default();
        assert_eq!(p.raw_for.as_secs(), 86_400);
        assert_eq!(p.minute_for.as_secs(), 2_592_000);
        assert_eq!(p.hour_for.as_secs(), 31_536_000);
        let cutoffs = p.cutoffs(1_000_000_000);
        assert_eq!(cutoffs.raw, 1_000_000_000 - 86_400);
        assert!(cutoffs.hour < cutoffs.minute);
        assert!(cutoffs.minute < cutoffs.raw);
    }

    #[test]
    fn raw_rows_collapse_into_the_right_minute_averages() {
        let mut conn = conn();
        // Two full minutes of raw data, 20 seconds apart.
        let samples = vec![
            cpu(1, 0, 10.0),
            cpu(1, 20, 20.0),
            cpu(1, 40, 30.0),
            cpu(1, 60, 100.0),
            cpu(1, 80, 200.0),
        ];
        insert_samples(&conn, &samples).unwrap();

        // now = 10_000 with a 1-hour raw window puts everything past the
        // cutoff, and the cutoff aligns to 6_600.
        let report = downsample(&mut conn, 10_000, &policy(1, 24, 240)).unwrap();
        assert_eq!(report.raw_collapsed, 5);
        assert_eq!(report.minute_written, 2);
        assert_eq!(report.deleted, 0);

        let rows = samples_between(&conn, 1, 0, 1_000).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].ts, 0);
        assert_eq!(rows[0].resolution, RESOLUTION_MINUTE);
        assert_eq!(rows[0].cpu_percent, Some(20.0)); // (10+20+30)/3
        assert_eq!(rows[0].mem_used_bytes, Some(200)); // (100+200+300)/3
        assert_eq!(rows[1].ts, 60);
        assert_eq!(rows[1].cpu_percent, Some(150.0)); // (100+200)/2
    }

    #[test]
    fn an_already_collapsed_row_is_not_collapsed_again() {
        let mut conn = conn();
        insert_samples(&conn, &[cpu(1, 0, 10.0), cpu(1, 30, 30.0)]).unwrap();

        let first = downsample(&mut conn, 10_000, &policy(1, 24, 240)).unwrap();
        assert_eq!(first.raw_collapsed, 2);
        assert_eq!(first.minute_written, 1);

        let second = downsample(&mut conn, 10_000, &policy(1, 24, 240)).unwrap();
        assert!(
            second.is_empty(),
            "second run should change nothing, got {second:?}"
        );

        let rows = samples_between(&conn, 1, 0, 1_000).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].cpu_percent, Some(20.0));
    }

    #[test]
    fn rows_newer_than_the_cutoff_are_left_alone() {
        let mut conn = conn();
        insert_samples(&conn, &[cpu(1, 100, 10.0), cpu(1, 9_000, 50.0)]).unwrap();

        // raw window of 1 hour at now=10_000 gives a cutoff of 6_400.
        let report = downsample(&mut conn, 10_000, &policy(1, 24, 240)).unwrap();
        assert_eq!(report.raw_collapsed, 1);

        let rows = samples_between(&conn, 1, 0, 100_000).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[1].ts, 9_000);
        assert_eq!(rows[1].resolution, RESOLUTION_RAW);
    }

    #[test]
    fn the_partial_bucket_at_the_cutoff_is_deferred() {
        let mut conn = conn();
        // now = 3_700, raw window 1 hour, so the cutoff is 100 and aligns
        // down to 60. The sample at 80 is older than the cutoff but shares a
        // minute with 100, so neither is collapsed yet.
        insert_samples(&conn, &[cpu(1, 80, 10.0), cpu(1, 100, 20.0)]).unwrap();
        let report = downsample(&mut conn, 3_700, &policy(1, 24, 240)).unwrap();
        assert!(report.is_empty(), "got {report:?}");

        // Once the whole minute is old enough, both collapse together.
        let later = downsample(&mut conn, 3_800, &policy(1, 24, 240)).unwrap();
        assert_eq!(later.raw_collapsed, 2);
        assert_eq!(later.minute_written, 1);
        let rows = samples_between(&conn, 1, 0, 1_000).unwrap();
        assert_eq!(rows[0].cpu_percent, Some(15.0));
    }

    #[test]
    fn minute_rows_collapse_into_hour_averages() {
        let mut conn = conn();
        let mut samples = Vec::new();
        // One hour of minute rows, plus one in the next hour.
        for minute in 0..60 {
            let mut sample = cpu(1, minute * 60, minute as f64);
            sample.resolution = RESOLUTION_MINUTE;
            samples.push(sample);
        }
        let mut next_hour = cpu(1, 3_600, 999.0);
        next_hour.resolution = RESOLUTION_MINUTE;
        samples.push(next_hour);
        insert_samples(&conn, &samples).unwrap();

        // now = 100_000, minute window 1 hour, so everything is past it.
        let report = downsample(&mut conn, 100_000, &policy(0, 1, 240)).unwrap();
        assert_eq!(report.minute_collapsed, 61);
        assert_eq!(report.hour_written, 2);

        let rows = samples_between(&conn, 1, 0, 100_000).unwrap();
        assert_eq!(rows.len(), 2);
        assert_eq!(rows[0].resolution, RESOLUTION_HOUR);
        assert_eq!(rows[0].ts, 0);
        assert_eq!(rows[0].cpu_percent, Some(29.5)); // mean of 0..=59
        assert_eq!(rows[1].ts, 3_600);
        assert_eq!(rows[1].cpu_percent, Some(999.0));
    }

    #[test]
    fn one_run_can_move_a_raw_row_all_the_way_to_an_hour_average() {
        let mut conn = conn();
        insert_samples(&conn, &[cpu(1, 0, 10.0), cpu(1, 30, 30.0)]).unwrap();
        // Raw and minute windows are both far behind now, hour window is not.
        let report = downsample(&mut conn, 1_000_000, &policy(1, 2, 1_000)).unwrap();
        assert_eq!(report.raw_collapsed, 2);
        assert_eq!(report.minute_written, 1);
        assert_eq!(report.minute_collapsed, 1);
        assert_eq!(report.hour_written, 1);

        let rows = samples_between(&conn, 1, 0, 1_000_000).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].resolution, RESOLUTION_HOUR);
        assert_eq!(rows[0].cpu_percent, Some(20.0));
    }

    #[test]
    fn rows_past_the_final_horizon_are_deleted() {
        let mut conn = conn();
        let mut old = cpu(1, 1_000, 10.0);
        old.resolution = RESOLUTION_HOUR;
        let mut recent = cpu(1, 900_000, 20.0);
        recent.resolution = RESOLUTION_HOUR;
        insert_samples(&conn, &[old, recent]).unwrap();

        // now = 1_000_000 with a 100-hour final horizon deletes anything
        // before 640_000.
        let report = downsample(&mut conn, 1_000_000, &policy(1, 2, 100)).unwrap();
        assert_eq!(report.deleted, 1);
        let rows = samples_between(&conn, 1, 0, 10_000_000).unwrap();
        assert_eq!(rows.len(), 1);
        assert_eq!(rows[0].ts, 900_000);
    }

    #[test]
    fn downsampling_an_empty_database_reports_nothing() {
        let mut conn = conn();
        let report = downsample(&mut conn, 1_000_000, &RetentionPolicy::default()).unwrap();
        assert!(report.is_empty());
    }

    #[test]
    fn keep_everything_never_collapses() {
        let mut conn = conn();
        insert_samples(&conn, &[cpu(1, 0, 10.0), cpu(1, 30, 30.0)]).unwrap();
        let report =
            downsample(&mut conn, i64::MAX / 2, &RetentionPolicy::keep_everything()).unwrap();
        assert!(report.is_empty(), "got {report:?}");
        assert_eq!(samples_between(&conn, 1, 0, 1_000).unwrap().len(), 2);
    }

    #[test]
    fn hosts_are_collapsed_independently() {
        let mut conn = conn();
        insert_samples(
            &conn,
            &[cpu(1, 0, 10.0), cpu(1, 30, 30.0), cpu(2, 0, 100.0)],
        )
        .unwrap();
        let report = downsample(&mut conn, 10_000, &policy(1, 24, 240)).unwrap();
        assert_eq!(report.raw_collapsed, 3);
        assert_eq!(report.minute_written, 2);
        assert_eq!(
            samples_between(&conn, 1, 0, 1_000).unwrap()[0].cpu_percent,
            Some(20.0)
        );
        assert_eq!(
            samples_between(&conn, 2, 0, 1_000).unwrap()[0].cpu_percent,
            Some(100.0)
        );
    }

    #[test]
    fn null_metrics_stay_null_through_a_collapse() {
        let mut conn = conn();
        let bare = MetricSample::new(1, 10);
        insert_samples(&conn, &[bare]).unwrap();
        downsample(&mut conn, 10_000, &policy(1, 24, 240)).unwrap();
        let rows = samples_between(&conn, 1, 0, 1_000).unwrap();
        assert_eq!(rows.len(), 1);
        assert!(rows[0].cpu_percent.is_none());
        assert!(rows[0].mem_used_bytes.is_none());
    }
}
