//! Aggregate queries: window summaries and fixed-width bucketed series.
//!
//! Both aggregate in SQL rather than pulling rows into Rust. A month of
//! one-minute data for one host is around 43,000 rows, and a dashboard redraw
//! that materialised all of them would stall the event loop.

use std::fmt::Write as _;

use rusqlite::{Connection, params};

use super::error::StoreError;
use super::types::{MetricKind, MetricStats, MetricSummary};

/// Upper bound on the bucket count of a series.
///
/// A sparkline is at most a few hundred cells wide. The cap exists so the
/// bucket index arithmetic in SQL, which multiplies a timestamp offset by the
/// bucket count, stays far from a 64-bit overflow.
pub const MAX_BUCKETS: usize = 100_000;

/// Minimum, maximum, mean and count of every metric for one host over
/// `[from_ts, to_ts]`, both ends inclusive.
///
/// Rows of any resolution are included, so a window spanning the raw/minute
/// boundary still summarises the whole period.
pub fn summary(
    conn: &Connection,
    host_id: i64,
    from_ts: i64,
    to_ts: i64,
) -> Result<MetricSummary, StoreError> {
    let mut columns = String::from("COUNT(*)");
    for metric in MetricKind::ALL {
        let expr = metric.sql_expr();
        // Writing into the buffer cannot fail; the Result is discarded rather
        // than unwrapped so this stays usable outside tests.
        let _ = write!(
            columns,
            ", MIN({expr}), MAX({expr}), AVG({expr}), COUNT({expr})"
        );
    }
    let sql = format!(
        "SELECT {columns} FROM metric_samples \
         WHERE host_id = ?1 AND ts >= ?2 AND ts <= ?3"
    );

    let mut stmt = conn.prepare_cached(&sql)?;
    let summary = stmt.query_row(params![host_id, from_ts, to_ts], |row| {
        let sample_count: i64 = row.get(0)?;
        let mut metrics = std::collections::BTreeMap::new();
        for (index, metric) in MetricKind::ALL.into_iter().enumerate() {
            let base = 1 + index * 4;
            let count: i64 = row.get(base + 3)?;
            if count == 0 {
                continue;
            }
            let min: Option<f64> = row.get(base)?;
            let max: Option<f64> = row.get(base + 1)?;
            let avg: Option<f64> = row.get(base + 2)?;
            // count > 0 guarantees the aggregates are non-null; the Option
            // unwrapping below is defensive against a NULL slipping through
            // rather than an expected branch.
            if let (Some(min), Some(max), Some(avg)) = (min, max, avg) {
                metrics.insert(
                    metric,
                    MetricStats {
                        min,
                        max,
                        avg,
                        count: count.max(0) as u64,
                    },
                );
            }
        }
        Ok(MetricSummary {
            sample_count: sample_count.max(0) as u64,
            metrics,
        })
    })?;
    Ok(summary)
}

/// A fixed-length series of `buckets` values covering `[from_ts, to_ts]`,
/// ready to hand to a ratatui `Sparkline`.
///
/// The window is split into `buckets` equal spans of whole seconds; each entry
/// is the mean of the metric over its span, or `None` when no sample landed
/// there. The length of the result always equals `buckets`, so the widget does
/// not have to reason about gaps.
///
/// A reversed or empty window yields `buckets` `None` entries rather than an
/// error, because the caller is usually a redraw that just needs something to
/// draw.
pub fn sparkline_series(
    conn: &Connection,
    host_id: i64,
    metric: MetricKind,
    from_ts: i64,
    to_ts: i64,
    buckets: usize,
) -> Result<Vec<Option<f64>>, StoreError> {
    if buckets == 0 {
        return Ok(Vec::new());
    }
    if buckets > MAX_BUCKETS {
        return Err(StoreError::InvalidArgument(format!(
            "bucket count {buckets} exceeds the maximum of {MAX_BUCKETS}"
        )));
    }

    let mut series = vec![None; buckets];
    // Inclusive bounds, so the window covers this many distinct seconds.
    let Some(span) = to_ts.checked_sub(from_ts).and_then(|d| d.checked_add(1)) else {
        return Ok(series);
    };
    if span <= 0 {
        return Ok(series);
    }

    let expr = metric.sql_expr();
    let sql = format!(
        "SELECT ((ts - ?2) * ?4) / ?5 AS bucket, AVG({expr}) FROM metric_samples \
         WHERE host_id = ?1 AND ts >= ?2 AND ts <= ?3 AND ({expr}) IS NOT NULL \
         GROUP BY bucket ORDER BY bucket"
    );

    let mut stmt = conn.prepare_cached(&sql)?;
    let rows = stmt.query_map(
        params![host_id, from_ts, to_ts, buckets as i64, span],
        |row| {
            let bucket: i64 = row.get(0)?;
            let value: Option<f64> = row.get(1)?;
            Ok((bucket, value))
        },
    )?;

    for row in rows {
        let (bucket, value) = row?;
        // Integer floor division over a window of `span` seconds keeps the
        // index below `buckets`; the bounds check guards against a stray row
        // rather than an expected case.
        if let Ok(index) = usize::try_from(bucket)
            && index < buckets
        {
            series[index] = value;
        }
    }
    Ok(series)
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use crate::store::metrics::insert_sample;
    use crate::store::schema;
    use crate::store::types::MetricSample;

    fn conn() -> Connection {
        let conn = Connection::open_in_memory().unwrap();
        schema::configure(&conn).unwrap();
        schema::ensure_schema(&conn).unwrap();
        conn
    }

    fn cpu_sample(host_id: i64, ts: i64, cpu: f64) -> MetricSample {
        let mut sample = MetricSample::new(host_id, ts);
        sample.cpu_percent = Some(cpu);
        sample
    }

    fn approx(a: f64, b: f64) {
        assert!((a - b).abs() < 1e-9, "expected {b}, got {a}");
    }

    #[test]
    fn summary_reports_min_max_avg_over_known_values() {
        let conn = conn();
        for (ts, cpu) in [(10, 10.0), (20, 20.0), (30, 60.0)] {
            insert_sample(&conn, &cpu_sample(1, ts, cpu)).unwrap();
        }
        let summary = summary(&conn, 1, 0, 100).unwrap();
        assert_eq!(summary.sample_count, 3);
        let stats = summary.stats(MetricKind::CpuPercent).unwrap();
        approx(stats.min, 10.0);
        approx(stats.max, 60.0);
        approx(stats.avg, 30.0);
        assert_eq!(stats.count, 3);
    }

    #[test]
    fn summary_of_a_single_sample_has_equal_min_max_avg() {
        let conn = conn();
        insert_sample(&conn, &cpu_sample(1, 5, 42.0)).unwrap();
        let summary = summary(&conn, 1, 0, 10).unwrap();
        assert_eq!(summary.sample_count, 1);
        let stats = summary.stats(MetricKind::CpuPercent).unwrap();
        approx(stats.min, 42.0);
        approx(stats.max, 42.0);
        approx(stats.avg, 42.0);
        assert_eq!(stats.count, 1);
    }

    #[test]
    fn summary_skips_metrics_the_host_never_reported() {
        let conn = conn();
        insert_sample(&conn, &cpu_sample(1, 5, 42.0)).unwrap();
        let summary = summary(&conn, 1, 0, 10).unwrap();
        assert!(summary.stats(MetricKind::GpuUtilPercent).is_none());
        assert!(summary.stats(MetricKind::TemperatureC).is_none());
    }

    #[test]
    fn summary_counts_only_rows_carrying_the_metric() {
        let conn = conn();
        insert_sample(&conn, &cpu_sample(1, 1, 10.0)).unwrap();
        insert_sample(&conn, &MetricSample::new(1, 2)).unwrap();
        let summary = summary(&conn, 1, 0, 10).unwrap();
        assert_eq!(summary.sample_count, 2, "both rows are in the window");
        assert_eq!(
            summary.stats(MetricKind::CpuPercent).unwrap().count,
            1,
            "only one row has a cpu reading"
        );
    }

    #[test]
    fn summary_of_an_empty_window_is_empty_not_an_error() {
        let conn = conn();
        insert_sample(&conn, &cpu_sample(1, 5, 42.0)).unwrap();
        let summary = summary(&conn, 1, 100, 200).unwrap();
        assert_eq!(summary.sample_count, 0);
        assert!(summary.metrics.is_empty());
    }

    #[test]
    fn summary_computes_derived_percentages() {
        let conn = conn();
        let mut sample = MetricSample::new(1, 1);
        sample.mem_used_bytes = Some(250);
        sample.mem_total_bytes = Some(1000);
        insert_sample(&conn, &sample).unwrap();
        let summary = summary(&conn, 1, 0, 10).unwrap();
        approx(summary.stats(MetricKind::MemUsedPercent).unwrap().avg, 25.0);
    }

    #[test]
    fn sparkline_returns_exactly_the_requested_bucket_count() {
        let conn = conn();
        insert_sample(&conn, &cpu_sample(1, 0, 1.0)).unwrap();
        for buckets in [1, 5, 17, 64] {
            let series =
                sparkline_series(&conn, 1, MetricKind::CpuPercent, 0, 99, buckets).unwrap();
            assert_eq!(series.len(), buckets);
        }
    }

    #[test]
    fn sparkline_leaves_empty_buckets_as_none() {
        let conn = conn();
        insert_sample(&conn, &cpu_sample(1, 0, 10.0)).unwrap();
        insert_sample(&conn, &cpu_sample(1, 90, 20.0)).unwrap();
        let series = sparkline_series(&conn, 1, MetricKind::CpuPercent, 0, 99, 10).unwrap();
        assert_eq!(series[0], Some(10.0));
        assert_eq!(series[9], Some(20.0));
        for (index, value) in series.iter().enumerate().take(9).skip(1) {
            assert_eq!(*value, None, "bucket {index} should be empty");
        }
    }

    #[test]
    fn sparkline_bucket_boundaries_are_exact() {
        let conn = conn();
        // Window [0, 99] over 10 buckets means each bucket spans 10 seconds.
        for ts in [0, 9, 10, 19, 20, 99] {
            insert_sample(&conn, &cpu_sample(1, ts, ts as f64)).unwrap();
        }
        let series = sparkline_series(&conn, 1, MetricKind::CpuPercent, 0, 99, 10).unwrap();
        approx(series[0].unwrap(), 4.5); // (0 + 9) / 2
        approx(series[1].unwrap(), 14.5); // (10 + 19) / 2
        approx(series[2].unwrap(), 20.0); // only ts 20
        assert_eq!(series[3], None);
        approx(series[9].unwrap(), 99.0); // the last second lands in the last bucket
    }

    #[test]
    fn sparkline_averages_within_a_bucket() {
        let conn = conn();
        insert_sample(&conn, &cpu_sample(1, 0, 0.0)).unwrap();
        insert_sample(&conn, &cpu_sample(1, 1, 100.0)).unwrap();
        let series = sparkline_series(&conn, 1, MetricKind::CpuPercent, 0, 9, 1).unwrap();
        assert_eq!(series.len(), 1);
        approx(series[0].unwrap(), 50.0);
    }

    #[test]
    fn sparkline_zero_buckets_is_an_empty_series() {
        let conn = conn();
        insert_sample(&conn, &cpu_sample(1, 0, 1.0)).unwrap();
        assert!(
            sparkline_series(&conn, 1, MetricKind::CpuPercent, 0, 99, 0)
                .unwrap()
                .is_empty()
        );
    }

    #[test]
    fn sparkline_reversed_window_is_all_none() {
        let conn = conn();
        insert_sample(&conn, &cpu_sample(1, 50, 1.0)).unwrap();
        let series = sparkline_series(&conn, 1, MetricKind::CpuPercent, 99, 0, 4).unwrap();
        assert_eq!(series, vec![None, None, None, None]);
    }

    #[test]
    fn sparkline_rejects_an_absurd_bucket_count() {
        let conn = conn();
        let err =
            sparkline_series(&conn, 1, MetricKind::CpuPercent, 0, 99, MAX_BUCKETS + 1).unwrap_err();
        assert!(matches!(err, StoreError::InvalidArgument(_)));
    }

    #[test]
    fn sparkline_ignores_other_hosts() {
        let conn = conn();
        insert_sample(&conn, &cpu_sample(1, 0, 10.0)).unwrap();
        insert_sample(&conn, &cpu_sample(2, 0, 90.0)).unwrap();
        let series = sparkline_series(&conn, 1, MetricKind::CpuPercent, 0, 9, 1).unwrap();
        approx(series[0].unwrap(), 10.0);
    }
}
