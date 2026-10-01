//! Context update endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Json;
use axum::extract::{Extension, Path, State};
use axum::response::Response;
use systemprompt_identifiers::ContextId;
use systemprompt_runtime::AppContext;

use super::super::responses::{api_error_response, single_response};
use systemprompt_agent::models::context::UpdateContextRequest;
use systemprompt_events::EventRouter;
use systemprompt_models::{ApiError, SystemEventBuilder};

pub async fn update_context(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<AppContext>,
    Path(context_id_str): Path<String>,
    Json(request): Json<UpdateContextRequest>,
) -> Response {
    let context_id = match ContextId::try_new(context_id_str) {
        Ok(id) => id,
        Err(e) => return api_error_response(ApiError::from(e)),
    };
    let context_repo = &ctx.a2a_repositories().contexts;
    let user_id = &req_ctx.auth.actor.user_id;

    match context_repo
        .update_context_name(&context_id, user_id, &request.name)
        .await
    {
        Ok(()) => {
            tracing::debug!(
                context_id = %context_id,
                user_id = %user_id,
                "Updated context"
            );

            match context_repo.get_context(&context_id, user_id).await {
                Ok(context) => {
                    let event =
                        SystemEventBuilder::context_updated(context_id.clone(), Some(request.name));
                    EventRouter::route_system(user_id, event)
                        .await
                        .into_local_logged();

                    single_response(context)
                },
                Err(e) => api_error_response(ApiError::internal(
                    "Context updated but failed to retrieve",
                    e,
                )),
            }
        },
        Err(e) => api_error_response(ApiError::from(e)),
    }
}
