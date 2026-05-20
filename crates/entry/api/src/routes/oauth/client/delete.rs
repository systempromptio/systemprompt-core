use axum::extract::{Extension, Path};
use axum::http::StatusCode;
use axum::response::{IntoResponse, Response};
use tracing::instrument;

use super::super::OAuthHttpError;
use super::super::extractors::OAuthRepo;
use systemprompt_models::RequestContext;

#[instrument(skip(repository, req_ctx), fields(client_id = %client_id))]
pub async fn delete_client(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repository): OAuthRepo,
    Path(client_id): Path<String>,
) -> Result<Response, OAuthHttpError> {
    let client_id = systemprompt_identifiers::ClientId::new(&client_id);
    let client = repository
        .find_client_by_id(&client_id)
        .await?
        .ok_or_else(|| OAuthHttpError::not_found(format!("Client with ID '{client_id}' not found")))?;

    repository.delete_client(&client_id).await?;

    tracing::info!(
        client_id = %client_id,
        client_name = ?client.name,
        deleted_by = %req_ctx.auth.actor.user_id,
        "OAuth client deleted"
    );
    Ok(StatusCode::NO_CONTENT.into_response())
}
