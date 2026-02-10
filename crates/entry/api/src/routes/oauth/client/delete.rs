
use axum::extract::{Extension, Path, State};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tracing::instrument;

use super::super::responses::{internal_error, not_found};
use systemprompt_models::RequestContext;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::OAuthState;

fn init_error(e: impl std::fmt::Display) -> Response {
    (
        StatusCode::INTERNAL_SERVER_ERROR,
        axum::Json(serde_json::json!({
            "error": "server_error",
            "error_description": format!("Repository initialization failed: {e}")
        })),
    )
        .into_response()
}

#[instrument(skip(state, req_ctx), fields(client_id = %client_id))]
pub async fn delete_client(
    Extension(req_ctx): Extension<RequestContext>,
    State(state): State<OAuthState>,
    Path(client_id): Path<String>,
) -> impl IntoResponse {
    let repository = match OAuthRepository::new(state.db_pool()) {
        Ok(r) => r,
        Err(e) => return init_error(e),
    };

    match repository.find_client_by_id(&client_id).await {
        Ok(Some(client)) => match repository.delete_client(&client_id).await {
            Ok(_) => {
                tracing::info!(
                    client_id = %client_id,
                    client_name = ?client.name,
                    deleted_by = %req_ctx.auth.user_id,
                    "OAuth client deleted"
                );
                StatusCode::NO_CONTENT.into_response()
            },
            Err(e) => {
                tracing::error!(
                    error = %e,
                    client_id = %client_id,
                    deleted_by = %req_ctx.auth.user_id,
                    "OAuth client deletion failed"
                );
                internal_error(format!("Failed to delete client: {e}"))
            },
        },
        Ok(None) => {
            tracing::info!(
                client_id = %client_id,
                reason = "Client not found",
                deleted_by = %req_ctx.auth.user_id,
                "OAuth client deletion failed"
            );
            not_found(format!("Client with ID '{client_id}' not found"))
        },
        Err(e) => {
            tracing::error!(
                error = %e,
                client_id = %client_id,
                deleted_by = %req_ctx.auth.user_id,
                "OAuth client deletion failed"
            );
            internal_error(format!("Failed to get client: {e}"))
        },
    }
}
