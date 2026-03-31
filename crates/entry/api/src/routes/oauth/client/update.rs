use axum::extract::{Extension, Path};
use axum::response::{IntoResponse, Json};
use tracing::instrument;

use super::super::extractors::OAuthRepo;
use super::super::responses::{bad_request, internal_error, not_found, single_response};
use systemprompt_models::RequestContext;
use systemprompt_oauth::clients::api::{OAuthClientResponse, UpdateOAuthClientRequest};

#[instrument(skip(repository, req_ctx, request), fields(client_id = %client_id))]
pub async fn update_client(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repository): OAuthRepo,
    Path(client_id): Path<String>,
    Json(request): Json<UpdateOAuthClientRequest>,
) -> impl IntoResponse {
    match repository.find_client_by_id(&client_id).await {
        Ok(Some(prev_client)) => {
            match repository
                .update_client(
                    &client_id,
                    request.name.as_deref(),
                    request.redirect_uris.as_deref(),
                    request.scopes.as_deref(),
                )
                .await
            {
                Ok(client) => {
                    tracing::info!(
                        client_id = %client_id,
                        client_name = ?client.name,
                        updated_by = %req_ctx.auth.user_id,
                        name_changed = request.name.is_some() && request.name.as_deref() != prev_client.name.as_deref(),
                        redirect_uris_changed = request.redirect_uris.is_some(),
                        scopes_changed = request.scopes.is_some(),
                        "OAuth client updated"
                    );
                    let response: OAuthClientResponse = client.into();
                    single_response(response)
                },
                Err(e) => {
                    tracing::error!(
                        error = %e,
                        client_id = %client_id,
                        updated_by = %req_ctx.auth.user_id,
                        "OAuth client update failed"
                    );
                    bad_request(&format!("Failed to update client: {e}"))
                },
            }
        },
        Ok(None) => {
            tracing::info!(
                client_id = %client_id,
                reason = "Client not found",
                updated_by = %req_ctx.auth.user_id,
                "OAuth client update failed"
            );
            not_found(&format!("Client with ID '{client_id}' not found"))
        },
        Err(e) => {
            tracing::error!(
                error = %e,
                client_id = %client_id,
                updated_by = %req_ctx.auth.user_id,
                "OAuth client update failed"
            );
            internal_error(&format!("Failed to get client: {e}"))
        },
    }
}
