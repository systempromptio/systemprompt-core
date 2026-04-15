use axum::extract::{Extension, Path};
use axum::http::StatusCode;
use axum::response::IntoResponse;
use tracing::instrument;

use super::super::extractors::OAuthRepo;
use super::super::responses::{internal_error, not_found};
use systemprompt_models::RequestContext;

#[instrument(skip(repository, req_ctx), fields(client_id = %client_id))]
pub async fn delete_client(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repository): OAuthRepo,
    Path(client_id): Path<String>,
) -> impl IntoResponse {
    let client_id = systemprompt_identifiers::ClientId::new(&client_id);
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
                internal_error(&format!("Failed to delete client: {e}"))
            },
        },
        Ok(None) => {
            tracing::info!(
                client_id = %client_id,
                reason = "Client not found",
                deleted_by = %req_ctx.auth.user_id,
                "OAuth client deletion failed"
            );
            not_found(&format!("Client with ID '{client_id}' not found"))
        },
        Err(e) => {
            tracing::error!(
                error = %e,
                client_id = %client_id,
                deleted_by = %req_ctx.auth.user_id,
                "OAuth client deletion failed"
            );
            internal_error(&format!("Failed to get client: {e}"))
        },
    }
}
