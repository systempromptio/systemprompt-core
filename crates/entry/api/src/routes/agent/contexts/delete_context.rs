use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use systemprompt_identifiers::ContextId;
use systemprompt_runtime::AppContext;

use super::super::responses::api_error_response;
use super::is_valid_context_id;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_events::EventRouter;
use systemprompt_models::{ApiError, SystemEventBuilder};

pub async fn delete_context(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<AppContext>,
    Path(context_id_str): Path<String>,
) -> Response {
    if !is_valid_context_id(&context_id_str) {
        return api_error_response(ApiError::bad_request(
            "Invalid context ID. Please select or create a valid conversation.",
        ));
    }

    let db_pool = ctx.db_pool().clone();
    let context_repo = match ContextRepository::new(&db_pool) {
        Ok(repo) => repo,
        Err(e) => {
            return api_error_response(ApiError::internal_error(format!("Database error: {e}")))
        },
    };
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
            api_error_response(ApiError::not_found(format!(
                "Failed to delete context: {e}"
            )))
        },
    }
}
