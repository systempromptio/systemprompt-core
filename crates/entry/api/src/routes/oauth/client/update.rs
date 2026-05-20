use axum::extract::{Extension, Path};
use axum::response::{Json, Response};
use tracing::instrument;

use super::super::OAuthHttpError;
use super::super::extractors::OAuthRepo;
use super::super::responses::single_response;
use systemprompt_models::RequestContext;
use systemprompt_oauth::clients::api::{OAuthClientResponse, UpdateOAuthClientRequest};

#[instrument(skip(repository, req_ctx, request), fields(client_id = %client_id))]
pub async fn update_client(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repository): OAuthRepo,
    Path(client_id): Path<String>,
    Json(request): Json<UpdateOAuthClientRequest>,
) -> Result<Response, OAuthHttpError> {
    let client_id = systemprompt_identifiers::ClientId::new(&client_id);
    let prev_client = repository
        .find_client_by_id(&client_id)
        .await?
        .ok_or_else(|| {
            OAuthHttpError::not_found(format!("Client with ID '{client_id}' not found"))
        })?;

    let client = repository
        .update_client(
            &client_id,
            request.name.as_deref(),
            request.redirect_uris.as_deref(),
            request.scopes.as_deref(),
        )
        .await
        .map_err(|e| OAuthHttpError::invalid_request(format!("Failed to update client: {e}")))?;

    tracing::info!(
        client_id = %client_id,
        client_name = ?client.name,
        updated_by = %req_ctx.auth.actor.user_id,
        name_changed = request.name.is_some() && request.name.as_deref() != prev_client.name.as_deref(),
        redirect_uris_changed = request.redirect_uris.is_some(),
        scopes_changed = request.scopes.is_some(),
        "OAuth client updated"
    );
    let response: OAuthClientResponse = client.into();
    Ok(single_response(response))
}
