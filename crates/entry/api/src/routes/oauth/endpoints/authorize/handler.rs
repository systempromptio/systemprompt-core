//! Authorization-endpoint request handlers.
//!
//! Implements the GET and POST `/authorize` flow: CSRF/state handling,
//! parameter validation, open-redirect-safe server-state issuance, and
//! rendering the `WebAuthn` challenge form. Password authentication is rejected
//! in favour of the `WebAuthn` flow.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::response_builder::{
    convert_form_to_query, generate_webauthn_form, is_user_consent_granted,
};
use super::validation::{SelfOrigins, validate_authorize_request, validate_oauth_parameters};
use super::{AuthorizeQuery, AuthorizeRequest};
use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;
use crate::services::request_base_url::RequestBaseUrl;
use axum::extract::{Extension, Form, Query, State};
use axum::response::{Html, IntoResponse, Response};
use systemprompt_models::{Config, RequestContext};
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::{OAuthRepository, StateBindingParams};
use systemprompt_oauth::services::generate_secure_token;
use systemprompt_oauth::services::validation::CsrfToken;
use tracing::instrument;

// Rejects open-redirect inputs: only same-origin, CR/LF-free absolute paths.
fn same_origin_return_path(client_state: &str) -> Option<String> {
    let raw = client_state.trim();
    if raw.is_empty()
        || !raw.starts_with('/')
        || raw.starts_with("//")
        || raw.starts_with("/\\")
        || raw.contains('\n')
        || raw.contains('\r')
    {
        return None;
    }
    Some(raw.to_owned())
}

async fn issue_server_state(
    repo: &OAuthRepository,
    return_to: &str,
    params: &AuthorizeQuery,
) -> Result<String, OAuthHttpError> {
    let server_state = generate_secure_token("state");
    let binding = StateBindingParams::builder(&server_state)
        .with_return_to(return_to)
        .with_client_id(&params.client_id)
        .with_redirect_uri(params.redirect_uri.as_deref().unwrap_or(""))
        .build();
    repo.store_state_binding(binding).await.map_err(|e| {
        tracing::error!(error = %e, "Failed to persist OAuth state binding");
        OAuthHttpError::server_error("Failed to persist authorization state")
    })?;
    Ok(server_state)
}

fn with_redirect_if_set(err: OAuthHttpError, query: &AuthorizeQuery) -> OAuthHttpError {
    if let Some(uri) = query.redirect_uri.as_deref() {
        err.with_redirect(uri, query.state.clone())
    } else {
        err
    }
}

fn require_csrf_token(params: &AuthorizeQuery) -> Result<CsrfToken, OAuthHttpError> {
    match params.state.as_deref() {
        None | Some("") => Err(OAuthHttpError::invalid_request(
            "CSRF token (state parameter) is required",
        )),
        Some(state_str) => CsrfToken::new(state_str).map_err(|_e| {
            OAuthHttpError::invalid_request("CSRF token (state parameter) is invalid")
        }),
    }
}

fn resolve_self_origins(base: &RequestBaseUrl) -> Result<SelfOrigins, OAuthHttpError> {
    let primary_origin = Config::get()
        .map_err(|e| {
            tracing::error!(error = %e, "Failed to load config for OAuth self-origin");
            OAuthHttpError::server_error("Configuration unavailable")
        })
        .and_then(|c| {
            reqwest::Url::parse(&c.api_external_url)
                .map(|u| u.origin())
                .map_err(|e| {
                    tracing::error!(
                        error = %e,
                        api_external_url = %c.api_external_url,
                        "api_external_url is not a valid URL — bootstrap validation should have caught this"
                    );
                    OAuthHttpError::server_error("Configuration invalid")
                })
        })?;
    Ok(SelfOrigins::new(primary_origin, base.origin().clone()))
}

async fn render_webauthn_form(
    repo: &OAuthRepository,
    params: &AuthorizeQuery,
    csrf_token: &CsrfToken,
    resolved_scope: &str,
) -> Result<Response, OAuthHttpError> {
    let form_state = match same_origin_return_path(csrf_token.as_str()) {
        Some(return_to) => issue_server_state(repo, &return_to, params).await?,
        None => csrf_token.as_str().to_owned(),
    };
    let mut form_params = params.clone();
    form_params.state = Some(form_state);
    let webauthn_form = generate_webauthn_form(&form_params, resolved_scope);
    Ok(Html(webauthn_form).into_response())
}

#[instrument(skip(repo, _req_ctx, params), fields(client_id = %params.client_id))]
pub async fn handle_authorize_get(
    State(state): State<OAuthState>,
    Extension(_req_ctx): Extension<RequestContext>,
    base: RequestBaseUrl,
    Query(params): Query<AuthorizeQuery>,
    OAuthRepo(repo): OAuthRepo,
) -> Result<Response, OAuthHttpError> {
    tracing::info!(
        client_id = %params.client_id,
        response_type = %params.response_type,
        redirect_uri = ?params.redirect_uri,
        requested_scopes = ?params.scope,
        state_present = params.state.is_some(),
        pkce_challenge_present = params.code_challenge.is_some(),
        code_challenge_method = ?params.code_challenge_method,
        "Authorization request received"
    );

    let csrf_token = require_csrf_token(&params)?;

    if params.response_type.is_empty() || params.client_id.as_str().is_empty() {
        let mut redirect_query = params.clone();
        redirect_query.state = Some(csrf_token.as_str().to_owned());
        return Err(with_redirect_if_set(
            OAuthHttpError::invalid_request("Validation error: Missing required parameters"),
            &redirect_query,
        ));
    }

    let self_origins = resolve_self_origins(&base)?;

    if let Err(validation_error) = validate_oauth_parameters(&params, &self_origins) {
        return Err(with_redirect_if_set(
            OAuthHttpError::invalid_request(validation_error),
            &params,
        ));
    }

    match validate_authorize_request(&state, &params, &repo).await {
        Ok(resolved_scope) => {
            tracing::info!(
                client_id = %params.client_id,
                resolved_scopes = %resolved_scope,
                redirect_uri = ?params.redirect_uri,
                state = ?params.state,
                "Authorization request validated"
            );

            render_webauthn_form(&repo, &params, &csrf_token, &resolved_scope).await
        },
        Err(error) => {
            tracing::info!(
                client_id = %params.client_id,
                denial_reason = %error,
                requested_scopes = ?params.scope,
                redirect_uri = ?params.redirect_uri,
                "Authorization request denied"
            );
            Err(with_redirect_if_set(
                OAuthHttpError::invalid_request(error.to_string()),
                &params,
            ))
        },
    }
}

#[instrument(skip(repo, _req_ctx, form), fields(client_id = %form.client_id))]
pub async fn handle_authorize_post(
    State(state): State<OAuthState>,
    Extension(_req_ctx): Extension<RequestContext>,
    OAuthRepo(repo): OAuthRepo,
    Form(form): Form<AuthorizeRequest>,
) -> Result<Response, OAuthHttpError> {
    let query = convert_form_to_query(&form);

    tracing::info!(
        client_id = %form.client_id,
        user_consent = ?form.user_consent,
        username_provided = form.username.is_some(),
        password_provided = form.password.is_some(),
        response_type = %form.response_type,
        "Authorization form submission received"
    );

    if let Err(error) = validate_authorize_request(&state, &query, &repo).await {
        return Err(with_redirect_if_set(
            OAuthHttpError::invalid_request(error.to_string()),
            &query,
        ));
    }

    if !is_user_consent_granted(&form) {
        tracing::info!(
            client_id = %form.client_id,
            denial_reason = "user_denied_consent",
            requested_scopes = ?form.scope,
            "User consent denied"
        );
        return Err(with_redirect_if_set(
            OAuthHttpError::access_denied("User denied the request"),
            &query,
        ));
    }

    tracing::info!(
        client_id = %form.client_id,
        attempted_method = "password_based",
        supported_method = "webauthn",
        "Unsupported authentication method attempted"
    );

    Err(with_redirect_if_set(
        OAuthHttpError::unsupported_grant_type(
            "Password authentication not supported. Use WebAuthn flow instead.",
        ),
        &query,
    ))
}
