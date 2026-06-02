use axum::extract::Extension;
use axum::http::StatusCode;
use axum::response::{Json, Response};
use bcrypt::{DEFAULT_COST, hash};
use tracing::instrument;
use uuid::Uuid;

use super::super::OAuthHttpError;
use super::super::extractors::OAuthRepo;
use super::super::responses::created_response;
use systemprompt_models::RequestContext;
use systemprompt_models::modules::ApiPaths;
use systemprompt_oauth::OauthError;
use systemprompt_oauth::clients::api::{CreateOAuthClientRequest, OAuthClientResponse};
use systemprompt_oauth::repository::CreateClientParams;

fn is_unique_violation(err: &OauthError) -> bool {
    if let OauthError::Repository(sqlx::Error::Database(db_err)) = err {
        db_err.is_unique_violation()
    } else {
        false
    }
}

#[instrument(skip(repository, req_ctx, request), fields(client_id = %request.client_id))]
pub async fn create_client(
    Extension(req_ctx): Extension<RequestContext>,
    OAuthRepo(repository): OAuthRepo,
    Json(request): Json<CreateOAuthClientRequest>,
) -> Result<Response, OAuthHttpError> {
    let client_secret = Uuid::new_v4().to_string();
    let client_secret_hash = hash(&client_secret, DEFAULT_COST)
        .map_err(|e| OAuthHttpError::server_error(format!("Failed to hash client secret: {e}")))?;

    let params = CreateClientParams {
        client_id: request.client_id.clone(),
        owner_user_id: req_ctx.auth.actor.user_id.clone(),
        client_secret_hash,
        client_name: request.name.clone(),
        redirect_uris: request.redirect_uris.clone(),
        grant_types: Some(vec![
            "authorization_code".to_owned(),
            "refresh_token".to_owned(),
        ]),
        response_types: Some(vec!["code".to_owned()]),
        scopes: request.scopes.clone(),
        token_endpoint_auth_method: Some("client_secret_basic".to_owned()),
        application_type: "web".to_owned(),
        client_uri: None,
        logo_uri: None,
        contacts: None,
    };

    let client = repository.create_client(params).await.map_err(|e| {
        if is_unique_violation(&e) {
            OAuthHttpError::invalid_client_metadata("Client with this ID already exists")
                .with_status(StatusCode::CONFLICT)
        } else {
            OAuthHttpError::invalid_request(format!("Failed to create client: {e}"))
        }
    })?;

    tracing::info!(
        client_id = %client.client_id,
        client_name = ?client.name,
        redirect_uris = ?request.redirect_uris,
        scopes = ?request.scopes,
        created_by = %req_ctx.auth.actor.user_id,
        "OAuth client created"
    );

    let location = ApiPaths::oauth_client_location(&client.client_id);
    let response: OAuthClientResponse = client.into();
    let mut response_json = serde_json::to_value(response)
        .map_err(|e| OAuthHttpError::server_error(format!("Failed to serialize response: {e}")))?;
    response_json["client_secret"] = serde_json::Value::String(client_secret);
    Ok(created_response(response_json, location))
}
