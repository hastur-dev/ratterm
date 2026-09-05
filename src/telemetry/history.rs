//! One host's past, ready to render.
//!
//! Split out of the ingest path because none of it touches the store: given a
//! series and a clock these are pure functions, and the dashboard's charts are
//! worth testing without a database behind them.

use std::time::Duration;

use crate::store::{MetricKind, MetricSummary};

/// The eight block heights a sparkline is drawn from, shortest first.
///
/// A gap — a bucket with no sample — is a space rather than the lowest block,
/// so "the host was quiet" and "the host was not there" do not look the same.
const SPARK_LEVELS: [char; 8] = ['▁', '▂', '▃', '▄', '▅', '▆', '▇', '█'];

/// The character used where a bucket has no data.
const SPARK_GAP: char = ' ';

/// One host's past, ready to render.
#[derive(Debug, Clone, PartialEq)]
pub struct HostHistory {
    /// False when there is no database, so the view can say why it is empty.
    pub durable: bool,
    /// The window the series and the summary cover.
    pub window: Duration,
    /// CPU percentage per bucket, oldest first. Empty means no history.
    pub cpu: Vec<Option<f64>>,
    /// Memory percentage per bucket, oldest first.
    pub memory: Vec<Option<f64>>,
    /// Minimum, maximum and mean over the window.
    pub summary: Option<MetricSummary>,
    /// Unix seconds the host was last seen, if it has stopped reporting.
    pub offline_since: Option<i64>,
}

impl HostHistory {
    /// Returns true when there is nothing to plot.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cpu.iter().all(Option::is_none) && self.memory.iter().all(Option::is_none)
    }

    /// Returns min, max and mean for one metric, as whole percentages.
    #[must_use]
    pub fn stats(&self, metric: MetricKind) -> Option<(f64, f64, f64)> {
        let stats = self.summary.as_ref()?.stats(metric)?;
        Some((stats.min, stats.max, stats.avg))
    }
}

/// Draws a series as a row of block characters.
///
/// Scaled between the smallest and largest value present rather than 0-100:
/// a host sitting between 3% and 7% CPU is a flat line at the bottom on an
/// absolute scale, which hides the shape the sparkline exists to show. A
/// series with no variation renders at the lowest level.
#[must_use]
pub fn sparkline(values: &[Option<f64>]) -> String {
    let present: Vec<f64> = values.iter().filter_map(|v| *v).collect();
    if present.is_empty() {
        return String::new();
    }

    let min = present.iter().copied().fold(f64::INFINITY, f64::min);
    let max = present.iter().copied().fold(f64::NEG_INFINITY, f64::max);
    let span = max - min;

    values
        .iter()
        .map(|value| match value {
            None => SPARK_GAP,
            Some(v) => {
                if span <= f64::EPSILON {
                    return SPARK_LEVELS[0];
                }
                #[allow(clippy::cast_possible_truncation, clippy::cast_sign_loss)]
                let index = (((v - min) / span) * (SPARK_LEVELS.len() - 1) as f64).round() as usize;
                SPARK_LEVELS[index.min(SPARK_LEVELS.len() - 1)]
            }
        })
        .collect()
}

/// Formats a "last seen" timestamp as an age, for the dashboard.
///
/// Returns `None` when the host is reporting, so the caller can leave the
/// line out rather than printing "offline for 0s" next to a live host.
#[must_use]
pub fn offline_for(offline_since: Option<i64>, now: i64) -> Option<String> {
    let since = offline_since?;
    let seconds = now.saturating_sub(since).max(0);

    Some(match seconds {
        0..=59 => format!("{seconds}s"),
        60..=3599 => format!("{}m", seconds / 60),
        3600..=86_399 => format!("{}h {}m", seconds / 3600, (seconds % 3600) / 60),
        _ => format!("{}d {}h", seconds / 86_400, (seconds % 86_400) / 3600),
    })
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn a_sparkline_of_nothing_is_empty() {
        assert_eq!(sparkline(&[]), "");
        assert_eq!(sparkline(&[None, None, None]), "");
    }

    #[test]
    fn a_sparkline_uses_the_full_height_range() {
        let line = sparkline(&[Some(0.0), Some(50.0), Some(100.0)]);
        assert_eq!(line.chars().count(), 3);
        let chars: Vec<char> = line.chars().collect();
        assert_eq!(
            chars[0], SPARK_LEVELS[0],
            "the lowest value is the shortest"
        );
        assert_eq!(
            chars[2],
            SPARK_LEVELS[SPARK_LEVELS.len() - 1],
            "the highest value is the tallest"
        );
        assert!(chars[1] > chars[0] && chars[1] < chars[2], "{line}");
    }

    #[test]
    fn a_sparkline_is_scaled_to_the_values_present() {
        // Between 3% and 7% on an absolute 0-100 scale is a flat line, which
        // is exactly the shape a sparkline should be showing.
        let line = sparkline(&[Some(3.0), Some(7.0), Some(5.0)]);
        let chars: Vec<char> = line.chars().collect();
        assert_eq!(chars[0], SPARK_LEVELS[0]);
        assert_eq!(chars[1], SPARK_LEVELS[SPARK_LEVELS.len() - 1]);
    }

    #[test]
    fn a_flat_sparkline_does_not_divide_by_zero() {
        let line = sparkline(&[Some(42.0), Some(42.0), Some(42.0)]);
        assert_eq!(line, "▁▁▁");
    }

    #[test]
    fn a_gap_is_blank_rather_than_the_lowest_block() {
        let line = sparkline(&[Some(10.0), None, Some(20.0)]);
        let chars: Vec<char> = line.chars().collect();
        assert_eq!(chars[1], SPARK_GAP, "a missing sample is not a low sample");
        assert_eq!(chars.len(), 3, "the gap still occupies its slot");
    }

    #[test]
    fn a_negative_value_still_plots() {
        // Temperature deltas and load can go below zero on some reporters.
        let line = sparkline(&[Some(-5.0), Some(0.0), Some(5.0)]);
        assert_eq!(line.chars().count(), 3);
        assert_eq!(line.chars().next(), Some(SPARK_LEVELS[0]));
    }

    #[test]
    fn a_reporting_host_has_no_offline_age() {
        assert_eq!(offline_for(None, 1_700_000_000), None);
    }

    #[test]
    fn an_offline_age_is_written_at_a_useful_scale() {
        let now = 1_700_000_000;
        assert_eq!(offline_for(Some(now - 30), now).as_deref(), Some("30s"));
        assert_eq!(offline_for(Some(now - 600), now).as_deref(), Some("10m"));
        assert_eq!(
            offline_for(Some(now - 7_260), now).as_deref(),
            Some("2h 1m")
        );
        assert_eq!(
            offline_for(Some(now - 180_000), now).as_deref(),
            Some("2d 2h")
        );
    }

    #[test]
    fn a_clock_that_went_backwards_does_not_report_a_negative_age() {
        let now = 1_700_000_000;
        assert_eq!(offline_for(Some(now + 500), now).as_deref(), Some("0s"));
    }
}
