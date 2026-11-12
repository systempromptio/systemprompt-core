//! OAuth client lookup endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::{Extension, Path};
use axum::response::Response;
use tracing::instrument;

use super::super::OAuthHttpError;
use super::super::extractors::OAuthRepo;
use super::super::responses::single_response;
use super::ownership::managed_client;
use systemprompt_models::RequestContext;
use systemprompt_oauth::clients::api::OAuthClientResponse;

#[instrument(skip(repository, req_ctx), fields(client_id = %client_id))]
pub async fn get_client(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repository): OAuthRepo,
    Path(client_id): Path<String>,
) -> Result<Response, OAuthHttpError> {
    let client_id = systemprompt_identifiers::ClientId::try_new(client_id)?;
    let client = managed_client(&repository, &req_ctx, &client_id).await?;

    tracing::info!(
        client_id = %client_id,
        client_name = ?client.name,
        requested_by = %req_ctx.auth.actor.user_id,
        "OAuth client retrieved"
    );
    let response: OAuthClientResponse = client.into();
    Ok(single_response(response))
}
