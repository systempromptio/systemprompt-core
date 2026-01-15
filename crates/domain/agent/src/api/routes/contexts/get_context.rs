use axum::extract::{Extension, Path, State};
use axum::response::IntoResponse;
use systemprompt_identifiers::ContextId;

use super::is_valid_context_id;
use crate::repository::context::ContextRepository;
use systemprompt_models::{ApiError, ApiErrorExt, SingleResponse};

pub async fn get_context(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<systemprompt_runtime::AppContext>,
    Path(context_id_str): Path<String>,
) -> impl IntoResponse {
    if !is_valid_context_id(&context_id_str) {
        return ApiError::bad_request(
            "Invalid context ID. Please select or create a valid conversation.",
        )
        .with_request_context(&req_ctx)
        .into_response();
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
            SingleResponse::new(context).into_response()
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to get context");
            ApiError::not_found(format!("Context not found: {e}"))
                .with_request_context(&req_ctx)
                .into_response()
        }
    }
}
