//! REST API route definitions.

use std::sync::Arc;

use axum::{
    routing::{delete, get, post, put},
    Router,
};

use super::{handlers, state::ApiState};

/// Creates the REST API router with all routes.
pub fn create_router(state: Arc<ApiState>) -> Router {
    Router::new()
        // Editor endpoints
        .route("/api/v1/editor/content", get(handlers::editor::get_content))
        .route("/api/v1/editor/content", put(handlers::editor::set_content))
        .route("/api/v1/editor/open", post(handlers::editor::open_file))
        .route("/api/v1/editor/save", post(handlers::editor::save_file))
        .route("/api/v1/editor/cursor", get(handlers::editor::get_cursor))
        .route("/api/v1/editor/cursor", put(handlers::editor::set_cursor))
        .route("/api/v1/editor/insert", post(handlers::editor::insert_text))
        .route("/api/v1/editor/file", get(handlers::editor::get_file))
        // Terminal endpoints
        .route(
            "/api/v1/terminal/send_keys",
            post(handlers::terminal::send_keys),
        )
        .route("/api/v1/terminal/buffer", get(handlers::terminal::get_buffer))
        .route("/api/v1/terminal/size", get(handlers::terminal::get_size))
        .route("/api/v1/terminal/cursor", get(handlers::terminal::get_cursor))
        .route("/api/v1/terminal/title", get(handlers::terminal::get_title))
        .route("/api/v1/terminal/clear", post(handlers::terminal::clear))
        .route(
            "/api/v1/terminal/scrollback",
            get(handlers::terminal::get_scrollback),
        )
        .route(
            "/api/v1/terminal/selection",
            get(handlers::terminal::get_selection),
        )
        .route("/api/v1/terminal/scroll", post(handlers::terminal::scroll))
        // Filesystem endpoints
        .route("/api/v1/fs/read", get(handlers::fs::read_file))
        .route("/api/v1/fs/write", post(handlers::fs::write_file))
        .route("/api/v1/fs/exists", get(handlers::fs::exists))
        .route("/api/v1/fs/is_dir", get(handlers::fs::is_dir))
        .route("/api/v1/fs/is_file", get(handlers::fs::is_file))
        .route("/api/v1/fs/list_dir", get(handlers::fs::list_dir))
        .route("/api/v1/fs/mkdir", post(handlers::fs::mkdir))
        .route("/api/v1/fs/remove", delete(handlers::fs::remove))
        .route("/api/v1/fs/rename", post(handlers::fs::rename))
        .route("/api/v1/fs/copy", post(handlers::fs::copy))
        // Command endpoints
        .route("/api/v1/commands/register", post(handlers::commands::register))
        .route(
            "/api/v1/commands/unregister",
            delete(handlers::commands::unregister),
        )
        .route("/api/v1/commands/list", get(handlers::commands::list))
        .route("/api/v1/commands/execute", post(handlers::commands::execute))
        // Event endpoints
        .route("/api/v1/events/stream", get(handlers::events::stream))
        // Layout endpoints
        .route("/api/v1/layout/state", get(handlers::layout::get_state))
        .route("/api/v1/layout/focus", post(handlers::layout::set_focus))
        .route("/api/v1/layout/toggle_ide", post(handlers::layout::toggle_ide))
        .route("/api/v1/layout/split", put(handlers::layout::set_split))
        // Tab endpoints
        .route("/api/v1/tabs/terminal", get(handlers::tabs::list_terminal))
        .route("/api/v1/tabs/editor", get(handlers::tabs::list_editor))
        .route(
            "/api/v1/tabs/terminal/new",
            post(handlers::tabs::new_terminal),
        )
        .route(
            "/api/v1/tabs/terminal/switch",
            post(handlers::tabs::switch_terminal),
        )
        .route(
            "/api/v1/tabs/terminal/close",
            delete(handlers::tabs::close_terminal),
        )
        // System endpoints
        .route("/api/v1/system/version", get(handlers::system::get_version))
        .route("/api/v1/system/status", get(handlers::system::get_status))
        .route("/api/v1/system/status", put(handlers::system::set_status))
        .route("/api/v1/system/cwd", get(handlers::system::get_cwd))
        .route("/api/v1/system/config", get(handlers::system::get_config))
        .route("/api/v1/system/theme", get(handlers::system::get_theme))
        .route("/api/v1/system/theme", put(handlers::system::set_theme))
        .route("/api/v1/system/themes", get(handlers::system::list_themes))
        .route("/api/v1/system/notify", post(handlers::system::notify))
        // Extension endpoints
        .route(
            "/api/v1/extensions/list",
            get(handlers::system::list_extensions),
        )
        .route(
            "/api/v1/extensions/health",
            get(handlers::system::health),
        )
        .route(
            "/api/v1/extensions/reload",
            post(handlers::system::reload_extension),
        )
        .with_state(state)
}
