use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use super::validation::validate_registration_token;
use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

pub async fn delete_client_configuration(
    OAuthRepo(repository): OAuthRepo,
    Path(client_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, OAuthHttpError> {
    validate_registration_token(&headers)?;

    let client_id = systemprompt_identifiers::ClientId::new(&client_id);
    repository
        .find_client_by_id(&client_id)
        .await?
        .ok_or_else(|| OAuthHttpError::invalid_client_metadata("Client not found"))?;

    repository.delete_client(&client_id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
