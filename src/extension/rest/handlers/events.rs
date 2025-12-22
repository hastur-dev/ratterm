//! Event-related REST API handlers (SSE streaming).

use std::convert::Infallible;
use std::sync::Arc;

use axum::{
    extract::State,
    response::sse::{Event, KeepAlive, Sse},
};
use tokio_stream::{wrappers::BroadcastStream, StreamExt};

use crate::extension::rest::{state::ApiState, types::ApiEvent};

/// GET /events/stream - Server-Sent Events stream.
pub async fn stream(
    State(state): State<Arc<ApiState>>,
) -> Sse<impl tokio_stream::Stream<Item = Result<Event, Infallible>>> {
    // Subscribe to the broadcast channel
    let rx = state.event_tx.subscribe();

    // Convert to a stream
    let stream = BroadcastStream::new(rx).filter_map(|result| {
        result.ok().map(|event| {
            let event_name = match &event {
                ApiEvent::FileOpen { .. } => "file_open",
                ApiEvent::FileSave { .. } => "file_save",
                ApiEvent::FileClose { .. } => "file_close",
                ApiEvent::FocusChanged { .. } => "focus_changed",
                ApiEvent::ThemeChanged { .. } => "theme_changed",
                ApiEvent::TerminalOutput { .. } => "terminal_output",
                ApiEvent::KeyPress { .. } => "key_press",
                ApiEvent::ExtensionLoaded { .. } => "extension_loaded",
                ApiEvent::ExtensionUnloaded { .. } => "extension_unloaded",
            };

            let data = serde_json::to_string(&event).unwrap_or_default();

            Ok::<_, Infallible>(Event::default().event(event_name).data(data))
        })
    });

    Sse::new(stream).keep_alive(KeepAlive::default())
}
