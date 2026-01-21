pub mod create_context;
pub mod delete_context;
pub mod events;
pub mod get_context;
pub mod list_contexts;
pub mod notifications;
pub mod update_context;
pub mod webhook;

pub use create_context::create_context;
pub use delete_context::delete_context;
pub use get_context::get_context;
pub use list_contexts::list_contexts;
pub use notifications::handle_context_notification;
pub use update_context::update_context;
pub use webhook::{broadcast_a2a_event, broadcast_agui_event, broadcast_context_event};

use axum::routing::{get, post, MethodRouter};
use axum::Router;
use systemprompt_runtime::AppContext;

const INVALID_CONTEXT_IDS: &[&str] = &["undefined", "null", "", "__CONTEXT_LOADING__"];

pub fn is_valid_context_id(context_id: &str) -> bool {
    !INVALID_CONTEXT_IDS.contains(&context_id)
}

pub fn router() -> Router<AppContext> {
    let context_root_methods: MethodRouter<AppContext> =
        get(list_contexts).post(create_context);

    let context_id_methods: MethodRouter<AppContext> =
        get(get_context).put(update_context).delete(delete_context);

    Router::new()
        .route("/", context_root_methods)
        .route("/{id}", context_id_methods)
        .route(
            "/{context_id}/tasks",
            get(super::tasks::list_tasks_by_context),
        )
        .route(
            "/{context_id}/artifacts",
            get(super::artifacts::list_artifacts_by_context),
        )
        .route(
            "/{context_id}/notifications",
            post(handle_context_notification),
        )
        .route("/{context_id}/events", post(events::forward_event))
}

pub fn webhook_router() -> Router<AppContext> {
    Router::new()
        .route("/broadcast", post(broadcast_context_event))
        .route("/agui", post(broadcast_agui_event))
        .route("/a2a", post(broadcast_a2a_event))
}
