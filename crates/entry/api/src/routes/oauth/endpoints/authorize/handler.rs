use super::response_builder::{
    convert_form_to_query, create_consent_denied_response, create_error_response,
    generate_webauthn_form, is_user_consent_granted,
};
use super::validation::{validate_authorize_request, validate_oauth_parameters};
use super::{AuthorizeQuery, AuthorizeRequest, AuthorizeResponse};
use axum::extract::{Extension, Form, Query, State};
use axum::http::StatusCode;
use axum::response::{Html, IntoResponse, Redirect};
use axum::Json;
use systemprompt_models::RequestContext;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::validation::CsrfToken;
use systemprompt_oauth::OAuthState;
use tracing::instrument;

#[instrument(skip(state, _req_ctx, params), fields(client_id = %params.client_id))]
pub async fn handle_authorize_get(
    Extension(_req_ctx): Extension<RequestContext>,
    Query(params): Query<AuthorizeQuery>,
    State(state): State<OAuthState>,
) -> impl IntoResponse {
    let repo = match OAuthRepository::new(state.db_pool()) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };

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
            return (
                StatusCode::BAD_REQUEST,
                Json(AuthorizeResponse {
                    code: None,
                    state: None,
                    error: Some("invalid_request".to_string()),
                    error_description: Some("CSRF token (state parameter) is required".to_string()),
                }),
            )
                .into_response();
        },
        Some(state_str) => match CsrfToken::new(state_str) {
            Ok(token) => token,
            Err(_) => {
                return (
                    StatusCode::BAD_REQUEST,
                    Json(AuthorizeResponse {
                        code: None,
                        state: params.state.clone(),
                        error: Some("invalid_request".to_string()),
                        error_description: Some(
                            "CSRF token (state parameter) is invalid".to_string(),
                        ),
                    }),
                )
                    .into_response();
            },
        },
    };

    if params.response_type.is_empty() || params.client_id.as_str().is_empty() {
        let e = "Missing required parameters";
        if let Some(redirect_uri) = &params.redirect_uri {
            let error_url = format!(
                "{}?error=invalid_request&error_description={}&state={}",
                redirect_uri,
                urlencoding::encode(&format!("Validation error: {e}")),
                csrf_token.as_str()
            );
            return Redirect::to(&error_url).into_response();
        }
        return (
            StatusCode::BAD_REQUEST,
            Json(AuthorizeResponse {
                code: None,
                state: params.state.clone(),
                error: Some("invalid_request".to_string()),
                error_description: Some(format!("Validation error: {e}")),
            }),
        )
            .into_response();
    }

    if let Err(validation_error) = validate_oauth_parameters(&params) {
        return create_error_response(
            &params,
            "invalid_request",
            &validation_error,
            StatusCode::BAD_REQUEST,
        );
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
            Html(webauthn_form).into_response()
        },
        Err(error) => {
            tracing::info!(
                client_id = %params.client_id,
                denial_reason = %error,
                requested_scopes = ?params.scope,
                redirect_uri = ?params.redirect_uri,
                "Authorization request denied"
            );

            if let Some(redirect_uri) = &params.redirect_uri {
                let error_url = format!(
                    "{}?error=invalid_request&error_description={}&state={}",
                    redirect_uri,
                    urlencoding::encode(&error.to_string()),
                    csrf_token.as_str()
                );
                return Redirect::to(&error_url).into_response();
            }
            (
                StatusCode::BAD_REQUEST,
                Json(AuthorizeResponse {
                    code: None,
                    state: params.state,
                    error: Some("invalid_request".to_string()),
                    error_description: Some(error.to_string()),
                }),
            )
                .into_response()
        },
    }
}

#[instrument(skip(state, _req_ctx, form), fields(client_id = %form.client_id))]
pub async fn handle_authorize_post(
    Extension(_req_ctx): Extension<RequestContext>,
    State(state): State<OAuthState>,
    Form(form): Form<AuthorizeRequest>,
) -> impl IntoResponse {
    let repo = match OAuthRepository::new(state.db_pool()) {
        Ok(r) => r,
        Err(e) => {
            return (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(serde_json::json!({"error": "server_error", "error_description": format!("Repository initialization failed: {}", e)})),
            ).into_response();
        },
    };
    let query = convert_form_to_query(&form);

    tracing::info!(
        client_id = %form.client_id,
        user_consent = ?form.user_consent,
        username_provided = form.username.is_some(),
        password_provided = form.password.is_some(),
        response_type = %form.response_type,
        "Authorization form submission received"
    );

    if let Err(error) = validate_authorize_request(&query, &repo).await.map(|_| ()) {
        tracing::info!(
            client_id = %form.client_id,
            validation_error = %error,
            "Authorization form validation failed"
        );
        return create_error_response(
            &query,
            "invalid_request",
            &error.to_string(),
            StatusCode::BAD_REQUEST,
        );
    }

    if !is_user_consent_granted(&form) {
        tracing::info!(
            client_id = %form.client_id,
            denial_reason = "user_denied_consent",
            requested_scopes = ?form.scope,
            "User consent denied"
        );
        return create_consent_denied_response(&query);
    }

    tracing::info!(
        client_id = %form.client_id,
        attempted_method = "password_based",
        supported_method = "webauthn",
        "Unsupported authentication method attempted"
    );

    create_error_response(
        &query,
        "unsupported_grant_type",
        "Password authentication not supported. Use WebAuthn flow instead.",
        StatusCode::BAD_REQUEST,
    )
}
