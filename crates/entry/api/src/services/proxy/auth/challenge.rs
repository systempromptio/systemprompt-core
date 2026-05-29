//! OAuth challenge construction for the proxy auth boundary.
//!
//! [`OAuthChallengeBuilder`] builds the `WWW-Authenticate: Bearer` 401/403
//! responses (per RFC 6750 and RFC 9728) that drive MCP and agent clients into
//! their OAuth discovery handshake, deriving the advertised `resource_metadata`
//! URL from the incoming request host. [`AuthValidator`] performs the bearer
//! check and [`challenge_or_error`] maps a failed check onto a [`ProxyError`].

use axum::body::Body;
use axum::http::header::{AUTHORIZATION, HOST};
use axum::http::{HeaderMap, StatusCode};
use axum::response::Response;
use serde_json::json;

use crate::services::proxy::backend::ProxyError;
use crate::services::request_base_url::resolve as resolve_request_base_url;
use systemprompt_models::RequestContext;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::modules::ApiPaths;
use systemprompt_oauth::services::AuthService;
use systemprompt_runtime::AppContext;

#[derive(Debug, Clone, Copy)]
pub(super) struct AuthValidator;

impl AuthValidator {
    pub(super) fn validate_service_access(
        headers: &HeaderMap,
        service_name: &str,
        req_context: Option<&RequestContext>,
    ) -> Result<AuthenticatedUser, StatusCode> {
        let result = AuthService::authorize_service_access(headers, service_name);

        if let Err(status) = &result {
            let trace_id =
                req_context.map_or_else(|| "unknown".to_owned(), |rc| rc.trace_id().to_string());
            tracing::warn!(service = %service_name, status = %status, trace_id = %trace_id, "auth failed");
        }

        result
    }
}

pub(super) struct ChallengeRequest<'a> {
    pub service_name: &'a str,
    pub resource_path: &'a str,
    pub headers: &'a HeaderMap,
    pub ctx: &'a AppContext,
    pub status_code: StatusCode,
    pub has_authorization: bool,
}

#[derive(Debug, Clone, Copy)]
pub struct OAuthChallengeBuilder;

impl OAuthChallengeBuilder {
    /// Build the `resource_metadata` URL advertised in the WWW-Authenticate
    /// header. Host-derives the base from the incoming request so the 401
    /// challenge agrees with the body of
    /// `/.well-known/oauth-protected-resource` — both must reflect
    /// whichever identity the client dialled in on (127.0.0.1 vs localhost
    /// vs configured public host), or the OAuth flow fails to round-trip.
    pub fn resource_metadata_url(
        headers: &HeaderMap,
        configured_api_external_url: &str,
        resource_path: &str,
    ) -> Result<String, url::ParseError> {
        let configured = url::Url::parse(configured_api_external_url)?;
        let raw_host = headers.get(HOST).and_then(|v| v.to_str().ok());
        let base = resolve_request_base_url(raw_host, &configured).into_string();
        Ok(format!(
            "{base}/.well-known/oauth-protected-resource{resource_path}"
        ))
    }

    pub(super) fn build_challenge_response(
        req: &ChallengeRequest<'_>,
    ) -> Result<Response<Body>, StatusCode> {
        let ChallengeRequest {
            service_name,
            resource_path,
            headers,
            ctx,
            status_code,
            has_authorization,
        } = *req;
        tracing::warn!(service = %service_name, status = %status_code, "Building OAuth challenge");

        let resource_metadata_url =
            Self::resource_metadata_url(headers, &ctx.config().api_external_url, resource_path)
                .map_err(|e| {
                    tracing::error!(error = %e, "api_external_url is not a valid URL");
                    StatusCode::INTERNAL_SERVER_ERROR
                })?;

        let (auth_header_value, error_body) = if status_code == StatusCode::UNAUTHORIZED {
            if has_authorization {
                let header = format!(
                    "Bearer realm=\"{service_name}\", \
                     resource_metadata=\"{resource_metadata_url}\", error=\"invalid_token\", \
                     error_description=\"The access token is missing or invalid\""
                );
                let body = json!({
                    "error": "invalid_token",
                    "error_description": "The access token is missing or invalid",
                    "server": service_name
                });
                (header, body)
            } else {
                // RFC 6750 §3: omit `error` on the no-credentials challenge so clients
                // know to start the OAuth flow rather than treat the request as rejected.
                let header = format!(
                    "Bearer realm=\"{service_name}\", \
                     resource_metadata=\"{resource_metadata_url}\""
                );
                (header, json!({}))
            }
        } else {
            let header = format!(
                "Bearer realm=\"{service_name}\", error=\"insufficient_scope\", \
                 error_description=\"The access token lacks required scope\""
            );
            let body = json!({
                "error": "insufficient_scope",
                "error_description": "The access token does not have the required scope for this resource",
                "server": service_name
            });
            (header, body)
        };

        Response::builder()
            .status(status_code)
            .header("Content-Type", "application/json")
            .header("WWW-Authenticate", auth_header_value)
            .body(Body::from(error_body.to_string()))
            .map_err(|e| {
                tracing::error!(error = %e, "Failed to build OAuth challenge response");
                StatusCode::INTERNAL_SERVER_ERROR
            })
    }
}

pub(crate) fn build_mcp_unknown_service_challenge(
    service_name: &str,
    headers: &HeaderMap,
    ctx: &AppContext,
    req_context: Option<&RequestContext>,
) -> Option<ProxyError> {
    let status_code =
        AuthValidator::validate_service_access(headers, service_name, req_context).err()?;
    let resource_path = ApiPaths::mcp_server_endpoint(service_name);
    let has_authorization = headers.get(AUTHORIZATION).is_some();
    Some(challenge_or_error(&ChallengeRequest {
        service_name,
        resource_path: &resource_path,
        headers,
        ctx,
        status_code,
        has_authorization,
    }))
}

pub(super) fn challenge_or_error(req: &ChallengeRequest<'_>) -> ProxyError {
    match OAuthChallengeBuilder::build_challenge_response(req) {
        Ok(challenge_response) => ProxyError::AuthChallenge(Box::new(challenge_response)),
        Err(status) if status == StatusCode::UNAUTHORIZED => ProxyError::AuthenticationRequired {
            service: req.service_name.to_owned(),
        },
        Err(_) => ProxyError::Forbidden {
            service: req.service_name.to_owned(),
        },
    }
}
