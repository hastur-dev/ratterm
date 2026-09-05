//! The shape of a container lifecycle event, and how one is read off the
//! Docker event stream.
//!
//! Split out of `events.rs` so the conversion — which is pure and carries most
//! of the rules — sits apart from the recorder that stores it.

use bollard::models::{EventMessage, EventMessageTypeEnum};

use crate::store::ContainerEvent;

/// The lifecycle actions worth recording.
///
/// Docker emits dozens of container actions, most of which say nothing about
/// whether a service is up: `exec_start`, `attach`, `resize` and friends fire
/// constantly and would bury the ones that matter.
pub const TRACKED_ACTIONS: [&str; 8] = [
    "create",
    "start",
    "restart",
    "stop",
    "kill",
    "die",
    "destroy",
    "health_status",
];

/// A lifecycle event, tagged with the host it came from.
///
/// [`ContainerEvent`] keys on the store's host row id, which does not exist
/// until a store is open. This is the shape the UI holds and the shape the
/// live ring keeps.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct FleetEvent {
    /// Fleet key: `None` for the local daemon, `Some(id)` for an SSH host.
    pub host_key: Option<u32>,
    /// The host's display name at the time the event arrived.
    pub host_label: String,
    /// Container id as the daemon reported it.
    pub container_id: String,
    /// Container name, when the event carried one.
    pub container_name: Option<String>,
    /// Unix seconds, UTC.
    pub ts: i64,
    /// Normalised action, one of [`TRACKED_ACTIONS`].
    pub action: String,
    /// Extra context: an exit code, a health verdict.
    pub detail: Option<String>,
}

impl FleetEvent {
    /// The store shape, once a host row id is known.
    #[must_use]
    pub fn to_container_event(&self, host_row: i64) -> ContainerEvent {
        ContainerEvent {
            event_id: None,
            host_id: host_row,
            container_id: self.container_id.clone(),
            container_name: self.container_name.clone(),
            ts: self.ts,
            event: self.action.clone(),
            detail: self.detail.clone(),
        }
    }

    /// One line for the events pane.
    #[must_use]
    pub fn summary(&self) -> String {
        let name = self
            .container_name
            .clone()
            .unwrap_or_else(|| self.container_id.chars().take(12).collect());
        match self.detail.as_deref() {
            Some(detail) if !detail.is_empty() => {
                format!("{} {} {} ({})", self.host_label, name, self.action, detail)
            }
            _ => format!("{} {} {}", self.host_label, name, self.action),
        }
    }
}

/// Splits a raw Docker action into an action and its detail.
///
/// Health events arrive as `health_status: healthy`; the verdict belongs in
/// the detail column, not in the action name, so a filter on `health_status`
/// finds all of them.
#[must_use]
pub fn split_action(raw: &str) -> (String, Option<String>) {
    match raw.split_once(':') {
        Some((action, detail)) => {
            let detail = detail.trim();
            (
                action.trim().to_string(),
                (!detail.is_empty()).then(|| detail.to_string()),
            )
        }
        None => (raw.trim().to_string(), None),
    }
}

/// True when an action is one this application records.
#[must_use]
pub fn is_tracked(action: &str) -> bool {
    let (name, _) = split_action(action);
    TRACKED_ACTIONS.contains(&name.as_str())
}

/// Converts one message from the Docker event stream.
///
/// Returns `None` for anything that is not a tracked container lifecycle
/// event, which is most of the stream.
#[must_use]
pub fn event_from_message(
    message: &EventMessage,
    host_key: Option<u32>,
    host_label: &str,
) -> Option<FleetEvent> {
    if message.typ != Some(EventMessageTypeEnum::CONTAINER) {
        return None;
    }

    let raw_action = message.action.as_deref()?;
    let (action, mut detail) = split_action(raw_action);
    if !TRACKED_ACTIONS.contains(&action.as_str()) {
        return None;
    }

    let actor = message.actor.as_ref()?;
    let container_id = actor.id.clone().filter(|id| !id.is_empty())?;
    let attributes = actor.attributes.clone().unwrap_or_default();
    let container_name = attributes.get("name").cloned().filter(|n| !n.is_empty());

    // `die` carries the exit code, which is the single most useful detail an
    // overnight event log can have.
    if detail.is_none()
        && let Some(code) = attributes.get("exitCode")
    {
        detail = Some(format!("exit {code}"));
    }

    Some(FleetEvent {
        host_key,
        host_label: host_label.to_string(),
        container_id,
        container_name,
        ts: message.time.unwrap_or(0),
        action,
        detail,
    })
}

#[cfg(test)]
#[allow(clippy::unwrap_used, clippy::expect_used)]
mod tests {
    use super::*;
    use bollard::models::EventActor;
    use std::collections::HashMap;

    fn message(action: &str, id: &str) -> EventMessage {
        EventMessage {
            typ: Some(EventMessageTypeEnum::CONTAINER),
            action: Some(action.to_string()),
            actor: Some(EventActor {
                id: Some(id.to_string()),
                attributes: Some(HashMap::from([("name".to_string(), "web".to_string())])),
            }),
            scope: None,
            time: Some(1_700_000_000),
            time_nano: None,
        }
    }

    #[test]
    fn a_plain_action_has_no_detail() {
        assert_eq!(split_action("start"), ("start".to_string(), None));
        assert_eq!(split_action("  die  "), ("die".to_string(), None));
    }

    #[test]
    fn a_health_action_splits_the_verdict_into_the_detail() {
        assert_eq!(
            split_action("health_status: healthy"),
            ("health_status".to_string(), Some("healthy".to_string()))
        );
        assert_eq!(
            split_action("health_status:"),
            ("health_status".to_string(), None)
        );
    }

    #[test]
    fn only_lifecycle_actions_are_tracked() {
        for action in TRACKED_ACTIONS {
            assert!(is_tracked(action), "{action} should be tracked");
        }
        assert!(is_tracked("health_status: unhealthy"));
        for noise in ["exec_start: sh", "attach", "resize", "top", "exec_create"] {
            assert!(!is_tracked(noise), "{noise} should not be tracked");
        }
    }

    #[test]
    fn a_container_start_becomes_an_event() {
        let converted = event_from_message(&message("start", "abc123"), Some(2), "rock5c")
            .expect("a tracked container event");
        assert_eq!(converted.action, "start");
        assert_eq!(converted.container_name.as_deref(), Some("web"));
        assert_eq!(converted.host_key, Some(2));
        assert_eq!(converted.ts, 1_700_000_000);
    }

    #[test]
    fn a_die_event_keeps_the_exit_code_as_its_detail() {
        let mut raw = message("die", "abc123");
        if let Some(actor) = raw.actor.as_mut()
            && let Some(attrs) = actor.attributes.as_mut()
        {
            attrs.insert("exitCode".to_string(), "137".to_string());
        }
        let converted = event_from_message(&raw, None, "Local").expect("a tracked container event");
        assert_eq!(converted.detail.as_deref(), Some("exit 137"));
        assert!(converted.summary().contains("exit 137"), "{converted:?}");
    }

    #[test]
    fn a_health_event_keeps_the_verdict_rather_than_the_exit_code() {
        let mut raw = message("health_status: unhealthy", "abc123");
        if let Some(actor) = raw.actor.as_mut()
            && let Some(attrs) = actor.attributes.as_mut()
        {
            attrs.insert("exitCode".to_string(), "0".to_string());
        }
        let converted = event_from_message(&raw, None, "Local").expect("a tracked event");
        assert_eq!(converted.action, "health_status");
        assert_eq!(converted.detail.as_deref(), Some("unhealthy"));
    }

    #[test]
    fn non_container_and_untracked_messages_are_ignored() {
        let mut image_event = message("pull", "nginx");
        image_event.typ = Some(EventMessageTypeEnum::IMAGE);
        assert!(event_from_message(&image_event, None, "Local").is_none());

        assert!(event_from_message(&message("exec_start: sh", "abc"), None, "Local").is_none());

        let mut no_action = message("start", "abc");
        no_action.action = None;
        assert!(event_from_message(&no_action, None, "Local").is_none());

        let mut no_actor = message("start", "abc");
        no_actor.actor = None;
        assert!(event_from_message(&no_actor, None, "Local").is_none());

        let mut no_id = message("start", "");
        no_id.actor = Some(EventActor {
            id: Some(String::new()),
            attributes: None,
        });
        assert!(event_from_message(&no_id, None, "Local").is_none());
    }

    #[test]
    fn an_event_with_no_name_falls_back_to_the_short_id_in_its_summary() {
        let mut raw = message("start", "0123456789abcdef0123");
        raw.actor = Some(EventActor {
            id: Some("0123456789abcdef0123".to_string()),
            attributes: None,
        });
        let converted = event_from_message(&raw, None, "Local").expect("a tracked event");
        assert!(converted.container_name.is_none());
        assert_eq!(converted.summary(), "Local 0123456789ab start");
    }
}
