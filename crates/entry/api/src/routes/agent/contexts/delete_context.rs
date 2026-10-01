//! Context deletion endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use systemprompt_identifiers::ContextId;
use systemprompt_runtime::AppContext;

use super::super::responses::api_error_response;
use systemprompt_events::EventRouter;
use systemprompt_models::{ApiError, SystemEventBuilder};

pub async fn delete_context(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<AppContext>,
    Path(context_id_str): Path<String>,
) -> Response {
    let context_id = match ContextId::try_new(context_id_str) {
        Ok(id) => id,
        Err(e) => return api_error_response(ApiError::from(e)),
    };
    let context_repo = &ctx.a2a_repositories().contexts;
    let user_id = &req_ctx.auth.actor.user_id;

    match context_repo.delete_context(&context_id, user_id).await {
        Ok(()) => {
            tracing::debug!(
                context_id = %context_id,
                user_id = %user_id,
                "Deleted context"
            );

            let event = SystemEventBuilder::context_deleted(context_id);
            EventRouter::route_system(user_id, event)
                .await
                .into_local_logged();

            StatusCode::NO_CONTENT.into_response()
        },
        Err(e) => api_error_response(ApiError::from(e)),
    }
}
