//! Authorization-endpoint request handlers.
//!
//! `GET /authorize` validates the request, looks for a person signed in to
//! this browser and acts on the [`decide`] outcome: a code at once, the
//! one-click consent page, the way to sign in (the deployment login page, or
//! the `WebAuthn` challenge page when none is configured), or an OIDC error
//! on the client's registered `redirect_uri`. The browser session is used
//! only for a request whose `redirect_uri` is present and registered.
//!
//! `POST /authorize` takes the decision posted from a page this endpoint
//! rendered, and only from this server's own origin. Deny answers
//! `access_denied`; Allow is the consent page's (see [`super::consent`]).
//! Neither accepts a password.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::consent::{Allow, allow, consent_page, require_same_origin};
use super::decision::{Decision, decide, silent_allowed};
use super::issue::{issue_code_redirect, response_issuer, session_grant};
use super::login_redirect::login_page_redirect_target;
use super::response_builder::{convert_form_to_query, generate_webauthn_form};
use super::session::{SignedInRequest, browser_sign_in};
use super::validation::{
    RegisteredRedirect, SelfOrigins, ValidatedAuthorizeRequest, resolve_registered_redirect,
    validate_authorize_request, validate_oauth_parameters,
};
use super::{AuthorizeQuery, AuthorizeRequest};
use crate::routes::oauth::{OAuthHttpError, internal};
use crate::services::request_base_url::RequestBaseUrl;
use axum::extract::{Extension, Form, OriginalUri, Query, State};
use axum::http::{HeaderMap, Uri};
use axum::response::{Html, IntoResponse, Redirect, Response};
use std::sync::Arc;
use systemprompt_manifest::Config;
use systemprompt_oauth::OAuthState;
use systemprompt_oauth::repository::OAuthRepository;
use systemprompt_oauth::services::validation::CsrfToken;
use tracing::instrument;

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

fn resolve_self_origins(
    config: &Config,
    base: &RequestBaseUrl,
) -> Result<SelfOrigins, OAuthHttpError> {
    let primary_origin = reqwest::Url::parse(&config.api_external_url)
        .map(|u| u.origin())
        .map_err(|e| internal::server_error("Configuration invalid", e))?;
    Ok(SelfOrigins::new(primary_origin, base.origin().clone()))
}

async fn validate_request(
    state: &OAuthState,
    repo: &OAuthRepository,
    params: &AuthorizeQuery,
    self_origins: &SelfOrigins,
) -> Result<ValidatedAuthorizeRequest, OAuthHttpError> {
    if params.response_type.is_empty() || params.client_id.as_str().is_empty() {
        return Err(OAuthHttpError::invalid_request(
            "Validation error: Missing required parameters",
        ));
    }
    if let Err(error) = validate_oauth_parameters(params, self_origins) {
        return Err(OAuthHttpError::invalid_request(error.to_string()).with_source(error));
    }
    validate_authorize_request(state, params, repo)
        .await
        .map_err(|error| {
            tracing::info!(
                client_id = %params.client_id,
                denial_reason = %error,
                requested_scopes = ?params.scope,
                redirect_uri = ?params.redirect_uri,
                "Authorization request denied"
            );
            OAuthHttpError::from(error)
        })
}

async fn decide_request(
    repo: &OAuthRepository,
    config: &Config,
    request: Option<&SignedInRequest<'_>>,
    params: &AuthorizeQuery,
    now: i64,
) -> Result<Decision, OAuthHttpError> {
    let (consent_covered, silent) = match request {
        Some(request) => {
            let client_id = &request.params.client_id;
            let declared = repo.is_declared_client(client_id).await?;
            let scopes = OAuthRepository::parse_scopes(&request.validated.scope);
            let covered = declared
                && repo
                    .consent_covers(&request.sign_in.user_id, client_id, &scopes)
                    .await?;
            (
                covered,
                silent_allowed(declared, request.redirect_uri, config.authorize),
            )
        },
        None => (false, false),
    };
    let decision = decide(
        params,
        request.map(|request| request.sign_in),
        consent_covered,
        silent,
        now,
    );
    tracing::info!(
        client_id = %params.client_id,
        signed_in = request.is_some(),
        consent_covered,
        ?decision,
        "Authorization request decided"
    );
    Ok(decision)
}

fn sign_in_response(
    config: &Config,
    params: &AuthorizeQuery,
    validated: &ValidatedAuthorizeRequest,
    original: &Uri,
    now: i64,
) -> Response {
    let return_path = original
        .path_and_query()
        .map_or_else(|| original.path().to_owned(), ToString::to_string);
    config
        .login_page_url
        .as_deref()
        .and_then(|url| login_page_redirect_target(url, params, &return_path, now))
        .map_or_else(
            || Html(generate_webauthn_form(config, params, validated)).into_response(),
            |target| Redirect::to(&target).into_response(),
        )
}

#[instrument(skip_all, fields(client_id = %params.client_id))]
#[expect(
    clippy::too_many_arguments,
    reason = "axum extractors: each argument is one request-scoped extractor"
)]
pub async fn handle_authorize_get(
    State(state): State<OAuthState>,
    Extension(config): Extension<Arc<Config>>,
    base: RequestBaseUrl,
    OriginalUri(original): OriginalUri,
    headers: HeaderMap,
    Query(params): Query<AuthorizeQuery>,
) -> Result<Response, OAuthHttpError> {
    tracing::info!(
        client_id = %params.client_id,
        response_type = %params.response_type,
        redirect_uri = ?params.redirect_uri,
        requested_scopes = ?params.scope,
        state_present = params.state.is_some(),
        pkce_challenge_present = params.code_challenge.is_some(),
        prompt = ?params.prompt,
        max_age = ?params.max_age,
        "Authorization request received"
    );
    let repo = state.oauth_repository();

    require_csrf_token(&params)?;
    let redirect = resolve_registered_redirect(repo, &params).await?;
    let issuer = response_issuer(&base);
    let refuse = |err: OAuthHttpError| {
        RegisteredRedirect::attach_if_registered(redirect.as_ref(), err)
            .with_redirect_issuer(issuer.as_str())
    };

    let self_origins = resolve_self_origins(&config, &base)?;
    let validated = validate_request(&state, repo, &params, &self_origins)
        .await
        .map_err(|err| RegisteredRedirect::attach_if_registered(redirect.as_ref(), err))?;

    let sign_in = match redirect.as_ref() {
        Some(_) => browser_sign_in(&state, &headers).await,
        None => None,
    };
    let request = sign_in
        .as_ref()
        .zip(redirect.as_ref())
        .map(|(sign_in, redirect)| SignedInRequest {
            params: &params,
            validated: &validated,
            sign_in,
            redirect_uri: redirect.uri(),
        });
    let now = chrono::Utc::now().timestamp();

    match (
        decide_request(repo, &config, request.as_ref(), &params, now).await?,
        request,
    ) {
        (Decision::PasskeyForm, _) => {
            Ok(Html(generate_webauthn_form(&config, &params, &validated)).into_response())
        },
        (Decision::SignIn, _) => Ok(sign_in_response(
            &config, &params, &validated, &original, now,
        )),
        (Decision::ConsentRequired, _) => Err(refuse(OAuthHttpError::consent_required(
            "The person has not allowed this client the requested scopes",
        ))),
        (Decision::InteractionRequired, _) => Err(refuse(OAuthHttpError::interaction_required(
            "This client is issued a code only after the person confirms",
        ))),
        (Decision::ConsentPage, Some(request)) => consent_page(repo, &request, now),
        (Decision::IssueCode, Some(request)) => {
            issue_code_redirect(repo, &session_grant(&request)?, &issuer).await
        },
        (Decision::LoginRequired | Decision::ConsentPage | Decision::IssueCode, _) => Err(refuse(
            OAuthHttpError::login_required("The person is not signed in as the request requires"),
        )),
    }
}

#[instrument(skip_all, fields(client_id = %form.client_id))]
pub async fn handle_authorize_post(
    State(state): State<OAuthState>,
    Extension(config): Extension<Arc<Config>>,
    base: RequestBaseUrl,
    headers: HeaderMap,
    Form(form): Form<AuthorizeRequest>,
) -> Result<Response, OAuthHttpError> {
    let query = convert_form_to_query(&form);
    tracing::info!(
        client_id = %form.client_id,
        user_consent = ?form.user_consent,
        response_type = %form.response_type,
        "Authorization form submission received"
    );
    let repo = state.oauth_repository();

    let self_origins = resolve_self_origins(&config, &base)?;
    require_same_origin(&headers, &self_origins)?;
    let issuer = response_issuer(&base);
    let redirect = resolve_registered_redirect(repo, &query).await?;

    match form.user_consent.as_deref() {
        Some("deny") => {
            let attach = |err| RegisteredRedirect::attach_if_registered(redirect.as_ref(), err);
            if let Err(error) = validate_authorize_request(&state, &query, repo).await {
                return Err(attach(OAuthHttpError::from(error)));
            }
            tracing::info!(client_id = %form.client_id, "User consent denied");
            Err(
                attach(OAuthHttpError::access_denied("User denied the request"))
                    .with_redirect_issuer(issuer),
            )
        },
        Some("allow") => {
            require_csrf_token(&query)?;
            let redirect = redirect.ok_or_else(|| {
                OAuthHttpError::invalid_request("redirect_uri is not registered for this client")
            })?;
            let validated = validate_request(&state, repo, &query, &self_origins).await?;
            allow(
                &state,
                &headers,
                Allow {
                    params: &query,
                    validated: &validated,
                    redirect_uri: redirect.uri(),
                    consent_token: form.consent_token.as_deref(),
                    issuer: &issuer,
                },
            )
            .await
        },
        _ => Err(OAuthHttpError::invalid_request(
            "user_consent must be 'allow' or 'deny'",
        )),
    }
}
