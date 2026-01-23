use axum::extract::{Extension, Path, State};
use axum::response::Response;
use systemprompt_identifiers::ContextId;
use systemprompt_runtime::AppContext;

use super::super::responses::{api_error_response, single_response};
use super::is_valid_context_id;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_models::{ApiError, ApiErrorExt};

pub async fn get_context(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<AppContext>,
    Path(context_id_str): Path<String>,
) -> Response {
    if !is_valid_context_id(&context_id_str) {
        return api_error_response(
            ApiError::bad_request(
                "Invalid context ID. Please select or create a valid conversation.",
            )
            .with_request_context(&req_ctx),
        );
    }

    let db_pool = ctx.db_pool().clone();
    let context_repo = ContextRepository::new(db_pool.clone());
    let user_id = &req_ctx.auth.user_id;
    let context_id = ContextId::new(&context_id_str);

    match context_repo.get_context(&context_id, user_id).await {
        Ok(context) => {
            tracing::debug!(
                context_id = %context_id,
                user_id = %user_id,
                "Retrieved context"
            );
            single_response(context)
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to get context");
            api_error_response(
                ApiError::not_found(format!("Context not found: {e}"))
                    .with_request_context(&req_ctx),
            )
        },
    }
}
