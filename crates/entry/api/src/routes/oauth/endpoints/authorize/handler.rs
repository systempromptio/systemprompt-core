use super::response_builder::{
    convert_form_to_query, generate_webauthn_form, is_user_consent_granted,
};
use super::validation::{validate_authorize_request, validate_oauth_parameters};
use super::{AuthorizeQuery, AuthorizeRequest};
use crate::routes::oauth::OAuthHttpError;
use crate::routes::oauth::extractors::OAuthRepo;
use axum::extract::{Extension, Form, Query};
use axum::response::{Html, IntoResponse, Response};
use systemprompt_models::RequestContext;
use systemprompt_oauth::services::validation::CsrfToken;
use tracing::instrument;

fn with_redirect_if_set(err: OAuthHttpError, query: &AuthorizeQuery) -> OAuthHttpError {
    if let Some(uri) = query.redirect_uri.as_deref() {
        err.with_redirect(uri, query.state.clone())
    } else {
        err
    }
}

#[instrument(skip(repo, _req_ctx, params), fields(client_id = %params.client_id))]
pub async fn handle_authorize_get(
    Extension(_req_ctx): Extension<RequestContext>,
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

    let csrf_token = match params.state.as_deref() {
        None | Some("") => {
            return Err(OAuthHttpError::invalid_request(
                "CSRF token (state parameter) is required",
            ));
        },
        Some(state_str) => CsrfToken::new(state_str).map_err(|_| {
            OAuthHttpError::invalid_request("CSRF token (state parameter) is invalid")
        })?,
    };

    if params.response_type.is_empty() || params.client_id.as_str().is_empty() {
        let mut redirect_query = params.clone();
        redirect_query.state = Some(csrf_token.as_str().to_string());
        return Err(with_redirect_if_set(
            OAuthHttpError::invalid_request("Validation error: Missing required parameters"),
            &redirect_query,
        ));
    }

    if let Err(validation_error) = validate_oauth_parameters(&params) {
        return Err(with_redirect_if_set(
            OAuthHttpError::invalid_request(validation_error),
            &params,
        ));
    }

    match validate_authorize_request(&params, &repo).await {
        Ok(resolved_scope) => {
            tracing::info!(
                client_id = %params.client_id,
                resolved_scopes = %resolved_scope,
                redirect_uri = ?params.redirect_uri,
                state = ?params.state,
                "Authorization request validated"
            );

            let webauthn_form = generate_webauthn_form(&params, &resolved_scope);
            Ok(Html(webauthn_form).into_response())
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

    if let Err(error) = validate_authorize_request(&query, &repo).await {
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
