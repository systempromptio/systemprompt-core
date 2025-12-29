use std::sync::Arc;

use axum::extract::{Extension, Path, State};
use axum::response::IntoResponse;
use tracing::instrument;

use crate::models::clients::api::OAuthClientResponse;
use crate::repository::OAuthRepository;
use systemprompt_models::{ApiError, RequestContext, SingleResponse};
use systemprompt_runtime::AppContext;

#[instrument(skip(ctx, req_ctx), fields(client_id = %client_id))]
pub async fn get_client(
    Extension(req_ctx): Extension<RequestContext>,
    State(ctx): State<AppContext>,
    Path(client_id): Path<String>,
) -> impl IntoResponse {
    let repository = match OAuthRepository::new(Arc::clone(ctx.db_pool())) {
        Ok(r) => r,
        Err(e) => {
            return (
                axum::http::StatusCode::INTERNAL_SERVER_ERROR,
                axum::Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };

    match repository.find_client_by_id(&client_id).await {
        Ok(Some(client)) => {
            tracing::info!(
                client_id = %client_id,
                client_name = ?client.name,
                requested_by = %req_ctx.auth.user_id,
                "OAuth client retrieved"
            );
            let response: OAuthClientResponse = client.into();
            SingleResponse::new(response).into_response()
        },
        Ok(None) => {
            tracing::info!(
                client_id = %client_id,
                reason = "Client not found",
                requested_by = %req_ctx.auth.user_id,
                "OAuth client retrieval failed"
            );
            ApiError::not_found(format!("Client with ID '{client_id}' not found")).into_response()
        },
        Err(e) => {
            tracing::error!(
                error = %e,
                client_id = %client_id,
                requested_by = %req_ctx.auth.user_id,
                "OAuth client retrieval failed"
            );
            ApiError::internal_error(format!("Failed to get client: {e}")).into_response()
        },
    }
}
