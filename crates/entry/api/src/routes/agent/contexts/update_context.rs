use axum::extract::{Extension, Path, State};
use axum::response::Response;
use axum::Json;
use systemprompt_identifiers::ContextId;
use systemprompt_runtime::AppContext;

use super::super::responses::{api_error_response, single_response};
use super::is_valid_context_id;
use systemprompt_agent::models::context::UpdateContextRequest;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_events::EventRouter;
use systemprompt_models::{ApiError, SystemEventBuilder};

pub async fn update_context(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<AppContext>,
    Path(context_id_str): Path<String>,
    Json(request): Json<UpdateContextRequest>,
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

                    single_response(context)
                },
                Err(e) => {
                    tracing::error!(error = %e, "Failed to retrieve updated context");
                    api_error_response(ApiError::internal_error(format!(
                        "Context updated but failed to retrieve: {}",
                        e
                    )))
                },
            }
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to update context");
            api_error_response(ApiError::not_found(format!(
                "Failed to update context: {e}"
            )))
        },
    }
}
