//! The event list view.

use std::cmp::Ordering;
use std::time::Duration;

use chrono::{DateTime, Utc};
use k8s_openapi::api::core::v1::Event;

use super::{age_at, age_display, count_of, micro_to_utc, name_of, namespace_of, to_utc};

/// An event as a list row.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct EventView {
    /// Event object name.
    pub name: String,
    /// Namespace the event was recorded in.
    pub namespace: String,
    /// `Normal` or `Warning`. Kept as a string because the API does not
    /// constrain it and clusters do add their own.
    pub event_type: String,
    /// Short machine-readable reason, for example `BackOff`.
    pub reason: Option<String>,
    /// The human-readable message.
    pub message: String,
    /// Kind of the object the event is about.
    pub object_kind: Option<String>,
    /// Name of the object the event is about.
    pub object_name: Option<String>,
    /// How many times this event has repeated. Defaults to 1.
    pub count: u32,
    /// When the event was last seen, from whichever of the three timestamp
    /// fields the cluster populated.
    pub last_seen: Option<DateTime<Utc>>,
    /// When the event was first seen.
    pub first_seen: Option<DateTime<Utc>>,
}

impl EventView {
    /// Builds the view from an API object.
    #[must_use]
    pub fn from_api(event: &Event) -> Self {
        let meta = &event.metadata;

        // Clusters populate different timestamp fields depending on which
        // event API recorded the event, so all three are tried in the order
        // kubectl uses.
        let last_seen = to_utc(event.last_timestamp.as_ref())
            .or_else(|| micro_to_utc(event.event_time.as_ref()))
            .or_else(|| to_utc(meta.creation_timestamp.as_ref()));

        Self {
            name: name_of(meta),
            namespace: namespace_of(meta),
            event_type: event
                .type_
                .clone()
                .filter(|t| !t.is_empty())
                .unwrap_or_else(|| "Normal".to_string()),
            reason: event.reason.clone().filter(|r| !r.is_empty()),
            message: event.message.clone().unwrap_or_default(),
            object_kind: event.involved_object.kind.clone().filter(|k| !k.is_empty()),
            object_name: event.involved_object.name.clone().filter(|n| !n.is_empty()),
            count: count_of(event.count).max(1),
            last_seen,
            first_seen: to_utc(event.first_timestamp.as_ref())
                .or_else(|| to_utc(meta.creation_timestamp.as_ref())),
        }
    }

    /// Returns how long ago the event was last seen, at `now`.
    #[must_use]
    pub fn age(&self, now: DateTime<Utc>) -> Option<Duration> {
        age_at(self.last_seen, now)
    }

    /// Returns the short age string for the list, or `-`.
    #[must_use]
    pub fn age_display(&self, now: DateTime<Utc>) -> String {
        age_display(self.last_seen, now)
    }

    /// Returns the object column, for example `Pod/api-0`.
    #[must_use]
    pub fn object_display(&self) -> String {
        match (self.object_kind.as_deref(), self.object_name.as_deref()) {
            (Some(kind), Some(name)) => format!("{kind}/{name}"),
            (Some(kind), None) => kind.to_string(),
            (None, Some(name)) => name.to_string(),
            (None, None) => "-".to_string(),
        }
    }

    /// True for events that call for attention.
    #[must_use]
    pub fn is_warning(&self) -> bool {
        self.event_type == "Warning"
    }
}

/// Orders events for a list: newest first, then namespace and name.
///
/// The timestamp alone is not unique — a burst of events shares a second — so
/// the object name breaks ties and keeps the order stable across refreshes.
/// Events with no timestamp sort last.
#[must_use]
pub fn compare_events(a: &EventView, b: &EventView) -> Ordering {
    match (a.last_seen, b.last_seen) {
        (Some(left), Some(right)) => right.cmp(&left),
        (Some(_), None) => Ordering::Less,
        (None, Some(_)) => Ordering::Greater,
        (None, None) => Ordering::Equal,
    }
    .then_with(|| a.namespace.cmp(&b.namespace))
    .then_with(|| a.name.cmp(&b.name))
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use k8s_openapi::api::core::v1::ObjectReference;
    use k8s_openapi::apimachinery::pkg::apis::meta::v1::ObjectMeta;

    use super::super::test_support::{micro_time_at, time_at, utc_at};
    use super::*;

    fn full_event() -> Event {
        Event {
            metadata: ObjectMeta {
                name: Some("api-0.17c9".to_string()),
                namespace: Some("web".to_string()),
                creation_timestamp: Some(time_at(900)),
                ..ObjectMeta::default()
            },
            type_: Some("Warning".to_string()),
            reason: Some("BackOff".to_string()),
            message: Some("Back-off restarting failed container".to_string()),
            count: Some(12),
            first_timestamp: Some(time_at(900)),
            last_timestamp: Some(time_at(1_000)),
            involved_object: ObjectReference {
                kind: Some("Pod".to_string()),
                name: Some("api-0".to_string()),
                ..ObjectReference::default()
            },
            ..Event::default()
        }
    }

    #[test]
    fn a_fully_populated_event_converts_every_field() {
        let view = EventView::from_api(&full_event());

        assert_eq!(view.name, "api-0.17c9");
        assert_eq!(view.namespace, "web");
        assert_eq!(view.event_type, "Warning");
        assert_eq!(view.reason.as_deref(), Some("BackOff"));
        assert_eq!(view.message, "Back-off restarting failed container");
        assert_eq!(view.object_kind.as_deref(), Some("Pod"));
        assert_eq!(view.object_name.as_deref(), Some("api-0"));
        assert_eq!(view.count, 12);
        assert_eq!(view.last_seen, Some(utc_at(1_000)));
        assert_eq!(view.first_seen, Some(utc_at(900)));
        assert_eq!(view.object_display(), "Pod/api-0");
        assert!(view.is_warning());
        assert_eq!(view.age_display(utc_at(1_030)), "30s");
        assert_eq!(view.age(utc_at(1_030)), Some(Duration::from_secs(30)));
    }

    #[test]
    fn a_minimal_event_converts_to_documented_defaults() {
        let view = EventView::from_api(&Event::default());

        assert_eq!(view.name, "");
        assert_eq!(view.namespace, "");
        assert_eq!(view.event_type, "Normal");
        assert!(view.reason.is_none());
        assert_eq!(view.message, "");
        assert!(view.object_kind.is_none());
        assert!(view.object_name.is_none());
        // An absent count means the event happened once.
        assert_eq!(view.count, 1);
        assert!(view.last_seen.is_none());
        assert!(view.first_seen.is_none());
        assert_eq!(view.object_display(), "-");
        assert!(!view.is_warning());
        assert_eq!(view.age_display(utc_at(10)), "-");
    }

    #[test]
    fn an_event_with_only_an_event_time_still_has_an_age() {
        let event = Event {
            event_time: Some(micro_time_at(2_000)),
            ..Event::default()
        };
        assert_eq!(EventView::from_api(&event).last_seen, Some(utc_at(2_000)));
    }

    #[test]
    fn an_event_with_only_a_creation_timestamp_falls_back_to_it() {
        let event = Event {
            metadata: ObjectMeta {
                creation_timestamp: Some(time_at(3_000)),
                ..ObjectMeta::default()
            },
            ..Event::default()
        };
        let view = EventView::from_api(&event);
        assert_eq!(view.last_seen, Some(utc_at(3_000)));
        assert_eq!(view.first_seen, Some(utc_at(3_000)));
    }

    #[test]
    fn an_unexpected_event_type_is_kept_verbatim() {
        let mut event = full_event();
        event.type_ = Some("Audit".to_string());
        let view = EventView::from_api(&event);
        assert_eq!(view.event_type, "Audit");
        assert!(!view.is_warning());
    }

    #[test]
    fn a_negative_count_is_treated_as_one() {
        let mut event = full_event();
        event.count = Some(-4);
        assert_eq!(EventView::from_api(&event).count, 1);
    }

    #[test]
    fn an_event_with_only_a_kind_or_only_a_name_still_renders() {
        let event = Event {
            involved_object: ObjectReference {
                kind: Some("Node".to_string()),
                ..ObjectReference::default()
            },
            ..Event::default()
        };
        assert_eq!(EventView::from_api(&event).object_display(), "Node");

        let event = Event {
            involved_object: ObjectReference {
                name: Some("worker-1".to_string()),
                ..ObjectReference::default()
            },
            ..Event::default()
        };
        assert_eq!(EventView::from_api(&event).object_display(), "worker-1");
    }

    #[test]
    fn events_sort_newest_first_then_by_namespace_and_name() {
        let mut views = [
            event_at("web", "b", Some(1_000)),
            event_at("web", "a", Some(2_000)),
            event_at("api", "c", Some(1_000)),
            event_at("web", "d", None),
        ];
        views.sort_by(compare_events);

        let keys: Vec<String> = views
            .iter()
            .map(|v| format!("{}/{}", v.namespace, v.name))
            .collect();
        assert_eq!(keys, vec!["web/a", "api/c", "web/b", "web/d"]);
    }

    #[test]
    fn the_event_comparator_is_a_total_order() {
        let newer = event_at("web", "a", Some(2_000));
        let older = event_at("web", "b", Some(1_000));
        assert_eq!(compare_events(&newer, &newer), Ordering::Equal);
        assert_eq!(compare_events(&newer, &older), Ordering::Less);
        assert_eq!(compare_events(&older, &newer), Ordering::Greater);

        let undated = event_at("web", "c", None);
        assert_eq!(compare_events(&newer, &undated), Ordering::Less);
        assert_eq!(compare_events(&undated, &newer), Ordering::Greater);
    }

    #[test]
    fn sorting_an_already_sorted_event_list_does_not_move_anything() {
        let mut views = [
            event_at("web", "a", Some(2_000)),
            event_at("web", "b", Some(1_000)),
        ];
        let before = views.clone();
        views.sort_by(compare_events);
        assert_eq!(views, before);
    }

    fn event_at(namespace: &str, name: &str, seconds: Option<i64>) -> EventView {
        let event = Event {
            metadata: ObjectMeta {
                name: Some(name.to_string()),
                namespace: Some(namespace.to_string()),
                ..ObjectMeta::default()
            },
            last_timestamp: seconds.map(time_at),
            ..Event::default()
        };
        EventView::from_api(&event)
    }
}
