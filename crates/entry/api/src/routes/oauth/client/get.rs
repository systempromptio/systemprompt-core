use axum::extract::{Extension, Path};
use axum::response::Response;
use tracing::instrument;

use super::super::OAuthHttpError;
use super::super::extractors::OAuthRepo;
use super::super::responses::single_response;
use systemprompt_models::RequestContext;
use systemprompt_oauth::clients::api::OAuthClientResponse;

#[instrument(skip(repository, req_ctx), fields(client_id = %client_id))]
pub async fn get_client(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repository): OAuthRepo,
    Path(client_id): Path<String>,
) -> Result<Response, OAuthHttpError> {
    let client_id = systemprompt_identifiers::ClientId::new(&client_id);
    let client = repository
        .find_client_by_id(&client_id)
        .await?
        .ok_or_else(|| {
            OAuthHttpError::not_found(format!("Client with ID '{client_id}' not found"))
        })?;

    tracing::info!(
        client_id = %client_id,
        client_name = ?client.name,
        requested_by = %req_ctx.auth.actor.user_id,
        "OAuth client retrieved"
    );
    let response: OAuthClientResponse = client.into();
    Ok(single_response(response))
}
