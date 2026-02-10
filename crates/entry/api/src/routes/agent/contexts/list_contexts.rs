use axum::extract::{Extension, State};
use axum::response::Response;
use systemprompt_runtime::AppContext;

use super::super::responses::{api_error_response, collection_response};
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_models::ApiError;

pub async fn list_contexts(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<AppContext>,
) -> Response {
    let db_pool = ctx.db_pool().clone();
    let context_repo = match ContextRepository::new(&db_pool) {
        Ok(repo) => repo,
        Err(e) => {
            return api_error_response(ApiError::internal_error(format!("Database error: {e}")))
        },
    };
    let user_id = &req_ctx.auth.user_id;

    match context_repo.list_contexts_with_stats(user_id).await {
        Ok(contexts) => {
            tracing::debug!(
                user_id = %user_id,
                count = contexts.len(),
                "Contexts listed"
            );
            collection_response(contexts)
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to list contexts");
            api_error_response(ApiError::internal_error(format!(
                "Failed to list contexts: {e}"
            )))
        },
    }
}
