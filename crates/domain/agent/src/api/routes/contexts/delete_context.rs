use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use systemprompt_identifiers::ContextId;

use super::is_valid_context_id;
use crate::repository::context::ContextRepository;
use systemprompt_core_events::EventRouter;
use systemprompt_models::{ApiError, SystemEventBuilder};

pub async fn delete_context(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<systemprompt_runtime::AppContext>,
    Path(context_id_str): Path<String>,
) -> impl IntoResponse {
    if !is_valid_context_id(&context_id_str) {
        return ApiError::bad_request(
            "Invalid context ID. Please select or create a valid conversation.",
        )
        .into_response();
    }

    let db_pool = ctx.db_pool().clone();
    let context_repo = ContextRepository::new(db_pool.clone());
    let user_id = &req_ctx.auth.user_id;
    let context_id = ContextId::new(&context_id_str);

    match context_repo.delete_context(&context_id, user_id).await {
        Ok(()) => {
            tracing::debug!(
                context_id = %context_id,
                user_id = %user_id,
                "Deleted context"
            );

            let event = SystemEventBuilder::context_deleted(context_id);
            EventRouter::route_system(user_id, event).await;

            StatusCode::NO_CONTENT.into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to delete context");
            ApiError::not_found(format!("Failed to delete context: {e}")).into_response()
        },
    }
}
