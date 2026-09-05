//! Threshold alert rules and their evaluation.
//!
//! Everything here is pure: a rule is data, and [`evaluate`] turns a sample
//! plus a rule list into the breaches it caused. Opening, closing and reading
//! stored alerts lives in [`super::alert_log`], so the decision of whether a
//! threshold was crossed can be tested without a database.

use serde::{Deserialize, Serialize};

use super::types::{MetricKind, MetricSample};

/// Metrics an alert rule can watch.
///
/// Narrower than [`MetricKind`] on purpose: a threshold on a raw byte count or
/// on uptime is not something an operator wants, and offering it would make
/// the rule list harder to read.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum AlertMetric {
    /// CPU busy percentage.
    CpuPercent,
    /// Memory in use as a percentage of total.
    MemUsedPercent,
    /// Disk in use as a percentage of total.
    DiskUsedPercent,
    /// Temperature in degrees Celsius.
    TemperatureC,
}

impl AlertMetric {
    /// The underlying series this metric reads.
    pub fn kind(self) -> MetricKind {
        match self {
            AlertMetric::CpuPercent => MetricKind::CpuPercent,
            AlertMetric::MemUsedPercent => MetricKind::MemUsedPercent,
            AlertMetric::DiskUsedPercent => MetricKind::DiskUsedPercent,
            AlertMetric::TemperatureC => MetricKind::TemperatureC,
        }
    }

    /// Stable name used in the stored rule key.
    pub fn as_str(self) -> &'static str {
        self.kind().as_str()
    }
}

/// Which side of the threshold counts as a breach.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum Comparison {
    /// Fires when the value is strictly greater than the threshold.
    Above,
    /// Fires when the value is strictly less than the threshold.
    Below,
}

impl Comparison {
    /// True when `value` breaches `threshold` on this side.
    ///
    /// Strict on both sides, so a rule at 90 does not fire at exactly 90 and a
    /// value sitting on the threshold does not flap between firing and clear.
    pub fn breaches(self, value: f64, threshold: f64) -> bool {
        match self {
            Comparison::Above => value > threshold,
            Comparison::Below => value < threshold,
        }
    }

    /// Stable name used in the stored rule key.
    pub fn as_str(self) -> &'static str {
        match self {
            Comparison::Above => "above",
            Comparison::Below => "below",
        }
    }
}

/// One threshold to watch.
#[derive(Debug, Clone, Copy, PartialEq, Serialize, Deserialize)]
pub struct AlertRule {
    /// What to measure.
    pub metric: AlertMetric,
    /// The boundary value.
    pub threshold: f64,
    /// Which side of the boundary fires.
    pub comparison: Comparison,
}

impl AlertRule {
    /// Builds a rule.
    pub fn new(metric: AlertMetric, comparison: Comparison, threshold: f64) -> Self {
        Self {
            metric,
            threshold,
            comparison,
        }
    }

    /// The identity written to `alerts.rule`.
    ///
    /// Two rules with the same metric, direction and threshold are the same
    /// alert, so editing the threshold opens a separate alert rather than
    /// silently reinterpreting the open one.
    pub fn key(&self) -> String {
        format!(
            "{}:{}:{}",
            self.metric.as_str(),
            self.comparison.as_str(),
            self.threshold
        )
    }
}

/// A rule breaching on one sample.
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct AlertFiring {
    /// Host the sample came from.
    pub host_id: i64,
    /// Timestamp of the sample that breached.
    pub ts: i64,
    /// The rule that breached.
    pub rule: AlertRule,
    /// The value that breached it.
    pub value: f64,
}

/// Rules breaching on `sample`.
///
/// Pure: it reads no database and no clock, so the same sample and rules
/// always give the same answer. A rule whose metric the sample does not carry
/// is skipped rather than treated as zero.
pub fn evaluate(sample: &MetricSample, rules: &[AlertRule]) -> Vec<AlertFiring> {
    let mut firings = Vec::new();
    for rule in rules {
        let Some(value) = rule.metric.kind().value_of(sample) else {
            continue;
        };
        if rule.comparison.breaches(value, rule.threshold) {
            firings.push(AlertFiring {
                host_id: sample.host_id,
                ts: sample.ts,
                rule: *rule,
                value,
            });
        }
    }
    firings
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;

    fn cpu_rule() -> AlertRule {
        AlertRule::new(AlertMetric::CpuPercent, Comparison::Above, 90.0)
    }

    #[test]
    fn rule_keys_separate_metric_direction_and_threshold() {
        assert_eq!(cpu_rule().key(), "cpu_percent:above:90");
        assert_eq!(
            AlertRule::new(AlertMetric::TemperatureC, Comparison::Below, 5.5).key(),
            "temperature_c:below:5.5"
        );
        assert_ne!(
            cpu_rule().key(),
            AlertRule::new(AlertMetric::CpuPercent, Comparison::Above, 80.0).key()
        );
    }

    #[test]
    fn comparisons_are_strict_on_both_sides() {
        assert!(Comparison::Above.breaches(91.0, 90.0));
        assert!(!Comparison::Above.breaches(90.0, 90.0));
        assert!(Comparison::Below.breaches(4.0, 5.0));
        assert!(!Comparison::Below.breaches(5.0, 5.0));
    }

    #[test]
    fn every_alert_metric_maps_to_a_series() {
        for metric in [
            AlertMetric::CpuPercent,
            AlertMetric::MemUsedPercent,
            AlertMetric::DiskUsedPercent,
            AlertMetric::TemperatureC,
        ] {
            assert_eq!(metric.as_str(), metric.kind().as_str());
        }
    }

    #[test]
    fn evaluate_is_pure_and_skips_missing_metrics() {
        let rules = [
            cpu_rule(),
            AlertRule::new(AlertMetric::TemperatureC, Comparison::Above, 70.0),
            AlertRule::new(AlertMetric::MemUsedPercent, Comparison::Above, 50.0),
        ];
        let mut sample = MetricSample::new(1, 10);
        sample.cpu_percent = Some(95.0);
        sample.mem_used_bytes = Some(100);
        sample.mem_total_bytes = Some(1000);

        let firings = evaluate(&sample, &rules);
        assert_eq!(firings.len(), 1, "only cpu breaches, got {firings:?}");
        assert_eq!(firings[0].value, 95.0);
        assert_eq!(firings[0].ts, 10);
        assert_eq!(firings[0].rule.metric, AlertMetric::CpuPercent);

        // Running it again changes nothing: no hidden state.
        assert_eq!(evaluate(&sample, &rules), firings);
    }

    #[test]
    fn evaluate_fires_on_each_supported_metric() {
        let mut sample = MetricSample::new(1, 0);
        sample.cpu_percent = Some(99.0);
        sample.mem_used_bytes = Some(950);
        sample.mem_total_bytes = Some(1000);
        sample.disk_used_bytes = Some(980);
        sample.disk_total_bytes = Some(1000);
        sample.temperature_c = Some(85.0);

        let rules = [
            AlertRule::new(AlertMetric::CpuPercent, Comparison::Above, 90.0),
            AlertRule::new(AlertMetric::MemUsedPercent, Comparison::Above, 90.0),
            AlertRule::new(AlertMetric::DiskUsedPercent, Comparison::Above, 90.0),
            AlertRule::new(AlertMetric::TemperatureC, Comparison::Above, 80.0),
        ];
        assert_eq!(evaluate(&sample, &rules).len(), 4);
    }

    #[test]
    fn an_empty_rule_list_never_fires() {
        let mut sample = MetricSample::new(1, 0);
        sample.cpu_percent = Some(100.0);
        assert!(evaluate(&sample, &[]).is_empty());
    }
}
