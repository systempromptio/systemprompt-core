use axum::extract::{Extension, State};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use axum::Json;

use crate::models::context::CreateContextRequest;
use crate::repository::context::ContextRepository;
use systemprompt_core_events::EventRouter;
use systemprompt_models::{ApiError, SingleResponse, SystemEventBuilder};

pub async fn create_context(
    Extension(req_ctx): Extension<systemprompt_models::RequestContext>,
    State(ctx): State<systemprompt_runtime::AppContext>,
    Json(request): Json<CreateContextRequest>,
) -> impl IntoResponse {
    let db_pool = ctx.db_pool().clone();
    let context_repo = ContextRepository::new(db_pool.clone());
    let user_id = &req_ctx.auth.user_id;

    let context_name = match request.name.as_deref().map(str::trim) {
        Some("") => return ApiError::bad_request("Context name cannot be empty").into_response(),
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
                        systemprompt_identifiers::ContextId::from(context.context_id.clone()),
                        context.name.clone(),
                    );
                    EventRouter::route_system(user_id, event).await;

                    (StatusCode::CREATED, Json(SingleResponse::new(context))).into_response()
                },
                Err(e) => {
                    tracing::error!(error = %e, "Failed to retrieve created context");
                    ApiError::internal_error(format!(
                        "Context created but failed to retrieve: {}",
                        e
                    ))
                    .into_response()
                },
            }
        },
        Err(e) => {
            tracing::error!(error = %e, "Failed to create context");
            ApiError::internal_error(format!("Failed to create context: {e}")).into_response()
        },
    }
}
