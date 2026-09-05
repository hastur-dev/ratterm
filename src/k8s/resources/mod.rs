//! Owned, UI-facing views of Kubernetes objects.
//!
//! Each view holds exactly the fields a list row needs and nothing else. The
//! `from_api` conversions are the only place in ratterm that reads a
//! `k8s-openapi` type, so a change in the API version is contained here.
//!
//! Every conversion is total: an object with almost every field absent
//! converts to a view with documented defaults rather than failing, because a
//! partially-populated object is normal (a pod that has not been scheduled has
//! no node, no IP and no container statuses).
//!
//! Timestamps are converted to `chrono` here. `k8s-openapi` uses `jiff`, but
//! the rest of ratterm — the log storage, the host status, the daemon metrics
//! — is on `chrono`, and one time type across the application is worth the
//! conversion.

mod event;
mod net;
mod node;
mod phase;
mod pod;
mod workload;

use std::collections::BTreeMap;
use std::time::Duration;

use chrono::{DateTime, Utc};
use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, ObjectMeta, Time};

pub use event::{EventView, compare_events};
pub use net::{ServicePortView, ServiceView, compare_services};
pub use node::{NodeReady, NodeView, compare_nodes};
pub use phase::PodPhase;
pub use pod::{PodView, compare_pods};
pub use workload::{DeploymentView, compare_deployments};

/// Seconds in a minute, hour, day and year, for [`format_age`].
const MINUTE: u64 = 60;
const HOUR: u64 = 60 * MINUTE;
const DAY: u64 = 24 * HOUR;
const YEAR: u64 = 365 * DAY;

/// Converts a Kubernetes `Time` into a chrono timestamp.
///
/// Returns `None` when the field is absent or the value is outside the range
/// chrono can represent, which only happens for corrupt data.
#[must_use]
pub fn to_utc(time: Option<&Time>) -> Option<DateTime<Utc>> {
    let ts = time?.0;
    let nanos = u32::try_from(ts.subsec_nanosecond()).unwrap_or(0);
    DateTime::from_timestamp(ts.as_second(), nanos)
}

/// Converts a Kubernetes `MicroTime` into a chrono timestamp.
#[must_use]
pub fn micro_to_utc(time: Option<&MicroTime>) -> Option<DateTime<Utc>> {
    let ts = time?.0;
    let nanos = u32::try_from(ts.subsec_nanosecond()).unwrap_or(0);
    DateTime::from_timestamp(ts.as_second(), nanos)
}

/// Returns how old an object is at `now`.
///
/// A creation timestamp in the future — the cluster's clock ahead of this
/// machine's — yields [`Duration::ZERO`] rather than an error, because a
/// negative age has no useful rendering and clock skew is not a fault the user
/// can act on.
#[must_use]
pub fn age_at(created: Option<DateTime<Utc>>, now: DateTime<Utc>) -> Option<Duration> {
    let created = created?;
    let seconds = now.signed_duration_since(created).num_seconds();
    Some(Duration::from_secs(u64::try_from(seconds).unwrap_or(0)))
}

/// Renders an age the way `kubectl` does: one unit, largest that fits.
///
/// Under a minute it counts seconds, then minutes, hours, days, and finally
/// years with the remaining days.
#[must_use]
pub fn format_age(age: Duration) -> String {
    let secs = age.as_secs();
    if secs < MINUTE {
        format!("{secs}s")
    } else if secs < HOUR {
        format!("{}m", secs / MINUTE)
    } else if secs < DAY {
        format!("{}h", secs / HOUR)
    } else if secs < YEAR {
        format!("{}d", secs / DAY)
    } else {
        format!("{}y{}d", secs / YEAR, (secs % YEAR) / DAY)
    }
}

/// Renders the age of an object, or `-` when it has no creation timestamp.
#[must_use]
pub fn age_display(created: Option<DateTime<Utc>>, now: DateTime<Utc>) -> String {
    age_at(created, now).map_or_else(|| "-".to_string(), format_age)
}

/// Returns the object's name, or an empty string when the API omitted it.
///
/// A listed object always has a name; the field is optional in the schema only
/// because the same type is used for create requests.
pub(crate) fn name_of(meta: &ObjectMeta) -> String {
    meta.name.clone().unwrap_or_default()
}

/// Returns the object's namespace, or an empty string for cluster-scoped
/// objects such as nodes.
pub(crate) fn namespace_of(meta: &ObjectMeta) -> String {
    meta.namespace.clone().unwrap_or_default()
}

/// Returns the object's labels, empty when it has none.
pub(crate) fn labels_of(meta: &ObjectMeta) -> BTreeMap<String, String> {
    meta.labels.clone().unwrap_or_default()
}

/// Clamps a signed API count to an unsigned one, treating a negative value as
/// zero. The API never sends negative counts; this keeps the conversion total.
pub(crate) fn count_of(value: Option<i32>) -> u32 {
    value.map_or(0, |v| u32::try_from(v).unwrap_or(0))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
pub(crate) mod test_support {
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::{MicroTime, Time};

    /// Builds a Kubernetes `Time` from a unix second.
    pub(crate) fn time_at(unix_seconds: i64) -> Time {
        Time(k8s_openapi::jiff::Timestamp::from_second(unix_seconds).expect("valid timestamp"))
    }

    /// Builds a Kubernetes `MicroTime` from a unix second.
    pub(crate) fn micro_time_at(unix_seconds: i64) -> MicroTime {
        MicroTime(k8s_openapi::jiff::Timestamp::from_second(unix_seconds).expect("valid timestamp"))
    }

    /// Builds a chrono timestamp from a unix second.
    pub(crate) fn utc_at(unix_seconds: i64) -> chrono::DateTime<chrono::Utc> {
        chrono::DateTime::from_timestamp(unix_seconds, 0).expect("valid timestamp")
    }
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::test_support::{micro_time_at, time_at, utc_at};
    use super::*;

    #[test]
    fn a_kubernetes_time_converts_to_chrono() {
        let converted = to_utc(Some(&time_at(1_700_000_000)));
        assert_eq!(converted, Some(utc_at(1_700_000_000)));
    }

    #[test]
    fn an_absent_time_converts_to_none() {
        assert!(to_utc(None).is_none());
        assert!(micro_to_utc(None).is_none());
    }

    #[test]
    fn a_micro_time_converts_to_chrono() {
        let converted = micro_to_utc(Some(&micro_time_at(1_700_000_000)));
        assert_eq!(converted, Some(utc_at(1_700_000_000)));
    }

    #[test]
    fn age_is_the_gap_between_creation_and_now() {
        let created = utc_at(1_000);
        let now = utc_at(1_090);
        assert_eq!(age_at(Some(created), now), Some(Duration::from_secs(90)));
    }

    #[test]
    fn age_of_an_object_with_no_timestamp_is_none() {
        assert!(age_at(None, utc_at(1_000)).is_none());
        assert_eq!(age_display(None, utc_at(1_000)), "-");
    }

    #[test]
    fn a_creation_time_in_the_future_clamps_to_zero() {
        // The cluster's clock is ahead of this machine's by an hour.
        let created = utc_at(10_000);
        let now = utc_at(6_400);
        assert_eq!(age_at(Some(created), now), Some(Duration::ZERO));
        assert_eq!(age_display(Some(created), now), "0s");
    }

    #[test]
    fn age_at_the_same_instant_is_zero() {
        let now = utc_at(5_000);
        assert_eq!(age_at(Some(now), now), Some(Duration::ZERO));
    }

    #[test]
    fn format_age_counts_seconds_below_a_minute() {
        assert_eq!(format_age(Duration::from_secs(0)), "0s");
        assert_eq!(format_age(Duration::from_secs(1)), "1s");
        assert_eq!(format_age(Duration::from_secs(59)), "59s");
    }

    #[test]
    fn format_age_switches_to_minutes_at_exactly_sixty_seconds() {
        assert_eq!(format_age(Duration::from_secs(60)), "1m");
        assert_eq!(format_age(Duration::from_secs(61)), "1m");
        assert_eq!(format_age(Duration::from_secs(3_599)), "59m");
    }

    #[test]
    fn format_age_switches_to_hours_at_exactly_one_hour() {
        assert_eq!(format_age(Duration::from_secs(3_600)), "1h");
        assert_eq!(format_age(Duration::from_secs(86_399)), "23h");
    }

    #[test]
    fn format_age_switches_to_days_at_exactly_one_day() {
        assert_eq!(format_age(Duration::from_secs(86_400)), "1d");
        assert_eq!(format_age(Duration::from_secs(YEAR - 1)), "364d");
    }

    #[test]
    fn format_age_switches_to_years_at_exactly_one_year() {
        assert_eq!(format_age(Duration::from_secs(YEAR)), "1y0d");
        assert_eq!(format_age(Duration::from_secs(YEAR + DAY * 3)), "1y3d");
        assert_eq!(format_age(Duration::from_secs(YEAR * 2 + DAY)), "2y1d");
    }

    #[test]
    fn object_metadata_helpers_default_when_fields_are_absent() {
        let empty = ObjectMeta::default();
        assert_eq!(name_of(&empty), "");
        assert_eq!(namespace_of(&empty), "");
        assert!(labels_of(&empty).is_empty());
    }

    #[test]
    fn object_metadata_helpers_read_populated_fields() {
        let meta = ObjectMeta {
            name: Some("api-0".to_string()),
            namespace: Some("web".to_string()),
            labels: Some(BTreeMap::from([("app".to_string(), "api".to_string())])),
            ..ObjectMeta::default()
        };
        assert_eq!(name_of(&meta), "api-0");
        assert_eq!(namespace_of(&meta), "web");
        assert_eq!(labels_of(&meta).get("app").map(String::as_str), Some("api"));
    }

    #[test]
    fn counts_clamp_absent_and_negative_values_to_zero() {
        assert_eq!(count_of(None), 0);
        assert_eq!(count_of(Some(0)), 0);
        assert_eq!(count_of(Some(7)), 7);
        assert_eq!(count_of(Some(-1)), 0);
    }
}
