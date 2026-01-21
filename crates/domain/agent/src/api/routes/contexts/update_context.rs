use axum::extract::{Extension, Path, State};
use axum::response::IntoResponse;
use axum::Json;
use systemprompt_identifiers::ContextId;

use super::is_valid_context_id;
use crate::models::context::UpdateContextRequest;
use crate::repository::context::ContextRepository;
use systemprompt_events::EventRouter;
use systemprompt_models::{ApiError, SingleResponse, SystemEventBuilder};

pub async fn update_context(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<systemprompt_runtime::AppContext>,
    Path(context_id_str): Path<String>,
    Json(request): Json<UpdateContextRequest>,
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
    let context_id = ContextId::from(context_id_str);

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
                    EventRouter::route_system(user_id, event).await;

                    SingleResponse::new(context).into_response()
                },
                Err(e) => {
                    tracing::error!(error = %e, "Failed to retrieve updated context");
                    ApiError::internal_error(format!(
                        "Context updated but failed to retrieve: {}",
                        e
                    ))
                    .into_response()
                },
            }
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to update context");
            ApiError::not_found(format!("Failed to update context: {e}")).into_response()
        },
    }
}
