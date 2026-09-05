//! Alert thresholds read from `.ratrc`.
//!
//! Written as one line per rule so a user can add a threshold without learning
//! a schema:
//!
//! ```text
//! alert.cpu = 90
//! alert.memory = 85
//! alert.disk = 85
//! alert.temperature = 80
//! ```
//!
//! A value of zero, or a line left out, means no rule for that metric. The
//! rules are evaluated on ingest, so a threshold applies to both collectors.

use crate::store::{AlertMetric, AlertRule, Comparison};

/// Highest threshold accepted for a percentage.
///
/// A percentage rule above 100 can never fire, which is more likely to be a
/// typo than an intention.
const MAX_PERCENT: f64 = 100.0;

/// Highest temperature threshold accepted, in degrees Celsius.
const MAX_TEMPERATURE: f64 = 150.0;

/// Thresholds as configured.
#[derive(Debug, Clone, Copy, Default, PartialEq)]
pub struct AlertSettings {
    /// Fire when CPU use is above this percentage.
    pub cpu_percent: Option<f64>,
    /// Fire when memory use is above this percentage.
    pub memory_percent: Option<f64>,
    /// Fire when disk use is above this percentage.
    pub disk_percent: Option<f64>,
    /// Fire when the hottest sensor is above this many degrees Celsius.
    pub temperature_c: Option<f64>,
}

impl AlertSettings {
    /// Applies one `.ratrc` line.
    ///
    /// Returns true if the key was one of ours, so the caller can tell an
    /// unknown setting from a handled one.
    pub fn apply(&mut self, key: &str, value: &str) -> bool {
        let Some(metric) = key.strip_prefix("alert.") else {
            return false;
        };

        let parsed = value.trim().parse::<f64>().ok();

        match metric.trim() {
            "cpu" => {
                self.cpu_percent = clamp_percent(parsed);
                true
            }
            "memory" | "mem" | "ram" => {
                self.memory_percent = clamp_percent(parsed);
                true
            }
            "disk" => {
                self.disk_percent = clamp_percent(parsed);
                true
            }
            "temperature" | "temp" => {
                self.temperature_c = clamp_temperature(parsed);
                true
            }
            _ => false,
        }
    }

    /// Returns true if any threshold is set.
    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.cpu_percent.is_none()
            && self.memory_percent.is_none()
            && self.disk_percent.is_none()
            && self.temperature_c.is_none()
    }

    /// Builds the rules the store evaluates.
    #[must_use]
    pub fn to_rules(self) -> Vec<AlertRule> {
        let mut rules = Vec::new();

        if let Some(threshold) = self.cpu_percent {
            rules.push(AlertRule::new(
                AlertMetric::CpuPercent,
                Comparison::Above,
                threshold,
            ));
        }
        if let Some(threshold) = self.memory_percent {
            rules.push(AlertRule::new(
                AlertMetric::MemUsedPercent,
                Comparison::Above,
                threshold,
            ));
        }
        if let Some(threshold) = self.disk_percent {
            rules.push(AlertRule::new(
                AlertMetric::DiskUsedPercent,
                Comparison::Above,
                threshold,
            ));
        }
        if let Some(threshold) = self.temperature_c {
            rules.push(AlertRule::new(
                AlertMetric::TemperatureC,
                Comparison::Above,
                threshold,
            ));
        }

        rules
    }
}

/// Accepts a percentage in (0, 100]; anything else means no rule.
fn clamp_percent(value: Option<f64>) -> Option<f64> {
    value.filter(|v| v.is_finite() && *v > 0.0 && *v <= MAX_PERCENT)
}

/// Accepts a temperature in (0, 150]; anything else means no rule.
fn clamp_temperature(value: Option<f64>) -> Option<f64> {
    value.filter(|v| v.is_finite() && *v > 0.0 && *v <= MAX_TEMPERATURE)
}

#[cfg(test)]
#[allow(clippy::expect_used, clippy::unwrap_used)]
mod tests {
    use super::*;

    #[test]
    fn nothing_configured_means_no_rules() {
        let settings = AlertSettings::default();
        assert!(settings.is_empty());
        assert!(settings.to_rules().is_empty());
    }

    #[test]
    fn each_metric_has_a_key() {
        let mut settings = AlertSettings::default();
        assert!(settings.apply("alert.cpu", "90"));
        assert!(settings.apply("alert.memory", "85"));
        assert!(settings.apply("alert.disk", "80"));
        assert!(settings.apply("alert.temperature", "75"));

        assert_eq!(settings.cpu_percent, Some(90.0));
        assert_eq!(settings.memory_percent, Some(85.0));
        assert_eq!(settings.disk_percent, Some(80.0));
        assert_eq!(settings.temperature_c, Some(75.0));
        assert!(!settings.is_empty());
        assert_eq!(settings.to_rules().len(), 4);
    }

    #[test]
    fn the_short_spellings_work() {
        let mut settings = AlertSettings::default();
        assert!(settings.apply("alert.ram", "70"));
        assert!(settings.apply("alert.temp", "60"));
        assert_eq!(settings.memory_percent, Some(70.0));
        assert_eq!(settings.temperature_c, Some(60.0));
    }

    #[test]
    fn a_key_that_is_not_ours_is_reported_as_such() {
        let mut settings = AlertSettings::default();
        assert!(!settings.apply("mode", "vim"));
        assert!(!settings.apply("alert.gpu", "90"));
        assert!(settings.is_empty());
    }

    #[test]
    fn zero_turns_a_rule_off() {
        let mut settings = AlertSettings::default();
        settings.apply("alert.cpu", "90");
        settings.apply("alert.cpu", "0");
        assert_eq!(settings.cpu_percent, None);
    }

    #[test]
    fn an_unparseable_value_turns_the_rule_off_rather_than_guessing() {
        let mut settings = AlertSettings::default();
        assert!(settings.apply("alert.cpu", "very high"));
        assert_eq!(settings.cpu_percent, None);
    }

    #[test]
    fn a_percentage_above_one_hundred_is_refused() {
        let mut settings = AlertSettings::default();
        settings.apply("alert.cpu", "150");
        assert_eq!(
            settings.cpu_percent, None,
            "a rule that can never fire is a typo"
        );
    }

    #[test]
    fn a_negative_threshold_is_refused() {
        let mut settings = AlertSettings::default();
        settings.apply("alert.disk", "-5");
        assert_eq!(settings.disk_percent, None);
    }

    #[test]
    fn a_temperature_above_the_ceiling_is_refused() {
        let mut settings = AlertSettings::default();
        settings.apply("alert.temperature", "500");
        assert_eq!(settings.temperature_c, None);
    }

    #[test]
    fn a_temperature_a_processor_can_actually_reach_is_accepted() {
        let mut settings = AlertSettings::default();
        settings.apply("alert.temperature", "95");
        assert_eq!(settings.temperature_c, Some(95.0));
    }

    #[test]
    fn whitespace_around_the_value_is_tolerated() {
        let mut settings = AlertSettings::default();
        settings.apply("alert.cpu", "  88  ");
        assert_eq!(settings.cpu_percent, Some(88.0));
    }

    #[test]
    fn rules_carry_the_configured_thresholds() {
        let mut settings = AlertSettings::default();
        settings.apply("alert.cpu", "91.5");
        let rules = settings.to_rules();
        assert_eq!(rules.len(), 1);
        assert!((rules[0].threshold - 91.5).abs() < f64::EPSILON);
        assert_eq!(rules[0].comparison, Comparison::Above);
    }
}
