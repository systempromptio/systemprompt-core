use axum::extract::{Extension, State};
use axum::response::Response;
use axum::Json;

use super::super::responses::{api_error_response, single_response_created};
use systemprompt_agent::models::context::CreateContextRequest;
use systemprompt_agent::repository::context::ContextRepository;
use systemprompt_events::EventRouter;
use systemprompt_models::{ApiError, ApiErrorExt, SystemEventBuilder};
use systemprompt_runtime::AppContext;

pub async fn create_context(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<AppContext>,
    Json(request): Json<CreateContextRequest>,
) -> Response {
    let db_pool = ctx.db_pool().clone();
    let context_repo = match ContextRepository::new(&db_pool) {
        Ok(repo) => repo,
        Err(e) => {
            return api_error_response(ApiError::internal_error(format!("Database error: {e}")))
        },
    };
    let user_id = &req_ctx.auth.user_id;

    let context_name = match request.name.as_deref().map(str::trim) {
        Some("") => {
            return api_error_response(
                ApiError::bad_request("Context name cannot be empty")
                    .with_request_context(&req_ctx),
            )
        },
        Some(name) => name.to_owned(),
        None => format!("Conversation {}", chrono::Utc::now().timestamp_millis()),
    };

    match context_repo
        .create_context(user_id, Some(&req_ctx.request.session_id), &context_name)
        .await
    {
        Ok(context_id) => {
            tracing::debug!(
                context_id = %context_id,
                user_id = %user_id,
                "Created context"
            );

            match context_repo.get_context(&context_id, user_id).await {
                Ok(context) => {
                    let event = SystemEventBuilder::context_created(
                        context.context_id.clone(),
                        context.name.clone(),
                    );
                    EventRouter::route_system(user_id, event).await;

                    single_response_created(context)
                },
                Err(e) => {
                    tracing::error!(error = %e, "Failed to retrieve created context");
                    api_error_response(
                        ApiError::internal_error(format!(
                            "Context created but failed to retrieve: {}",
                            e
                        ))
                        .with_request_context(&req_ctx),
                    )
                },
            }
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to create context");
            api_error_response(
                ApiError::internal_error(format!("Failed to create context: {e}"))
                    .with_request_context(&req_ctx),
            )
        },
    }
}
