//! Context lookup endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Extension, Path, State};
use axum::response::Response;
use systemprompt_identifiers::ContextId;
use systemprompt_runtime::AppContext;

use super::super::responses::{api_error_response, single_response};
use systemprompt_models::{ApiError, ApiErrorExt};

pub async fn get_context(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<AppContext>,
    Path(context_id_str): Path<String>,
) -> Response {
    let context_id = match ContextId::try_new(context_id_str) {
        Ok(id) => id,
        Err(e) => return api_error_response(ApiError::from(e).with_request_context(&req_ctx)),
    };
    let context_repo = &ctx.a2a_repositories().contexts;
    let user_id = &req_ctx.auth.actor.user_id;

    match context_repo.get_context(&context_id, user_id).await {
        Ok(context) => {
            tracing::debug!(
                context_id = %context_id,
                user_id = %user_id,
                "Retrieved context"
            );
            single_response(context)
        },
        Err(e) => api_error_response(ApiError::from(e).with_request_context(&req_ctx)),
    }
}
