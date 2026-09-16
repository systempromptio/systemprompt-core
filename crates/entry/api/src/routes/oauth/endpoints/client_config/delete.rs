//! RFC 7592 client-configuration delete endpoint.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::extract::Path;
use axum::http::{HeaderMap, StatusCode};
use axum::response::{IntoResponse, Response};

use super::validation::authenticate_client_configuration;
use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;

pub async fn delete_client_configuration(
    OAuthRepo(repository): OAuthRepo,
    Path(client_id): Path<String>,
    headers: HeaderMap,
) -> Result<Response, OAuthHttpError> {
    let client_id = systemprompt_identifiers::ClientId::new(&client_id);
    authenticate_client_configuration(&repository, &headers, &client_id).await?;

    repository.delete_client(&client_id).await?;
    Ok(StatusCode::NO_CONTENT.into_response())
}
