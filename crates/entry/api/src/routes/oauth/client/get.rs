use axum::extract::{Extension, Path};
use axum::response::IntoResponse;
use tracing::instrument;

use super::super::extractors::OAuthRepo;
use super::super::responses::{internal_error, not_found, single_response};
use systemprompt_models::RequestContext;
use systemprompt_oauth::clients::api::OAuthClientResponse;

#[instrument(skip(repository, req_ctx), fields(client_id = %client_id))]
pub async fn get_client(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repository): OAuthRepo,
    Path(client_id): Path<String>,
) -> impl IntoResponse {
    match repository.find_client_by_id(&client_id).await {
        Ok(Some(client)) => {
            tracing::info!(
                client_id = %client_id,
                client_name = ?client.name,
                requested_by = %req_ctx.auth.user_id,
                "OAuth client retrieved"
            );
            let response: OAuthClientResponse = client.into();
            single_response(response)
        },
        Ok(None) => {
            tracing::info!(
                client_id = %client_id,
                reason = "Client not found",
                requested_by = %req_ctx.auth.user_id,
                "OAuth client retrieval failed"
            );
            not_found(&format!("Client with ID '{client_id}' not found"))
        },
        Err(e) => {
            tracing::error!(
                error = %e,
                client_id = %client_id,
                requested_by = %req_ctx.auth.user_id,
                "OAuth client retrieval failed"
            );
            internal_error(&format!("Failed to get client: {e}"))
        },
    }
}
