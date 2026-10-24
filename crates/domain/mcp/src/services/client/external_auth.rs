//! Per-user bearer resolution for external MCP servers.
//!
//! An `external` MCP server backed by a third-party identity provider declares
//! an [`ExternalAuth`] block naming a relative accessor endpoint. The accessor
//! is served by an extension that banks the calling user's provider token; core
//! `GET`s it with the user's systemprompt JWT and receives a fresh bearer for
//! *that* user. The bearer (plus any static configured headers) is then
//! injected onto the outbound MCP request while the systemprompt credential is
//! withheld.
//!
//! A connector on a machine grant (`client_credentials`, `app_installation`) is
//! served by core itself: no user, no broker, one identity per deployment.
//!
//! `fetch_external_bearer` asks the credential broker for a server's bearer on
//! behalf of the JWT's user. The request names the MCP server
//! (`x-systemprompt-mcp-server`) and, when the request context carries one, the
//! caller purpose (`x-systemprompt-caller`). The broker may refuse by purpose:
//! a service account's credential can be scoped to the workflows allowed to use
//! it. Trust rule: the purpose is set by the issuer of a credential (an API
//! key's `purpose`, a JWT's `purpose` claim minted in-process), never by its
//! bearer. It comes only from `RequestContext::caller_purpose()`; an inbound
//! `x-systemprompt-caller` header is stripped at the API edge and never read.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;

use http::{HeaderName, HeaderValue};
use systemprompt_manifest::Config;
use systemprompt_models::RequestContext;
use systemprompt_models::mcp::ExternalAuth;
use systemprompt_security::credential::TokenCache;

use super::validation::rewrite_url_for_internal_use;
use crate::error::{McpDomainError, McpDomainResult};

pub const BROKER_SECRET_KEY: &str = "mcp_credential_broker_secret";
pub const MCP_SERVER_HEADER: &str = systemprompt_identifiers::headers::MCP_SERVER;
pub const CALLER_HEADER: &str = systemprompt_identifiers::headers::CALLER_PURPOSE;

#[derive(serde::Deserialize)]
struct AccessorResponse {
    access_token: String,
}

#[derive(serde::Deserialize)]
struct AccessorError {
    error: String,
}

const ACCESSOR_ERROR_MAX_CHARS: usize = 240;

pub(super) async fn resolve_external_bearer(
    connector_tokens: &TokenCache,
    ext: &ExternalAuth,
    context: &RequestContext,
    server: &str,
) -> McpDomainResult<String> {
    if let Some(token) = machine_grant_bearer(connector_tokens, ext, server).await? {
        return Ok(token);
    }
    let Some(jwt) = context.auth_token() else {
        return Err(McpDomainError::AuthRequired(format!(
            "external MCP server '{server}' requires an authenticated user to resolve its bearer"
        )));
    };

    let base = Config::get()?.api_external_url.clone();
    let accessor = accessor_url(&base, &ext.token_endpoint);
    // Why: the accessor authenticates this process by the broker secret; a
    // call without it would be refused (or, worse, served to an unbrokered
    // caller), so a missing secret is a configuration error here.
    let secrets = systemprompt_config::SecretsBootstrap::get().map_err(|e| {
        McpDomainError::invalid_configuration(
            format!("credential broker secret unavailable for external MCP server '{server}'"),
            e,
        )
    })?;
    let broker_secret = secrets
        .get(BROKER_SECRET_KEY)
        .filter(|secret| !secret.is_empty())
        .ok_or_else(|| {
            McpDomainError::Configuration(format!(
                "secret `{BROKER_SECRET_KEY}` is not configured; external MCP server '{server}' \
                 cannot resolve its bearer"
            ))
        })?;
    fetch_external_bearer(
        &accessor,
        jwt.as_str(),
        broker_secret,
        server,
        context.caller_purpose(),
    )
    .await
}

async fn machine_grant_bearer(
    connector_tokens: &TokenCache,
    ext: &ExternalAuth,
    server: &str,
) -> McpDomainResult<Option<String>> {
    let Ok(deployment) = crate::services::deployment::DeploymentService::get_deployment(server)
    else {
        return Ok(None);
    };
    let Some(connector) = deployment.connector.filter(|c| c.grant.is_machine()) else {
        return Ok(None);
    };
    let token =
        systemprompt_security::connector_grant::machine_token(connector_tokens, server, &connector)
            .await
            .map_err(|error| {
                tracing::warn!(server, %error, "connector machine grant failed");
                McpDomainError::ExternalAuthUnavailable {
                    server: server.to_owned(),
                    message: error.to_string(),
                }
            })?;
    // Why: a connector's resolved ExternalAuth carries an empty scheme because
    // the broker answers with a complete header value; a minted machine token
    // is bare, so the scheme is added here.
    Ok(Some(if ext.scheme.trim().is_empty() {
        format!("Bearer {token}")
    } else {
        token
    }))
}

pub fn accessor_url(api_external_url: &str, token_endpoint: &str) -> String {
    let accessor = format!(
        "{}{}",
        api_external_url.trim_end_matches('/'),
        token_endpoint
    );
    rewrite_url_for_internal_use(&accessor)
}

pub async fn fetch_external_bearer(
    accessor: &str,
    jwt: &str,
    broker_secret: &str,
    server: &str,
    caller_purpose: Option<&str>,
) -> McpDomainResult<String> {
    let client = systemprompt_client::guarded_client(
        &systemprompt_client::GuardedClientConfig::default().with_max_redirects(0),
    )
    .map_err(|e| {
        McpDomainError::transport(format!("token accessor client failed for '{server}'"), e)
    })?;
    let mut request = client
        .get(accessor)
        .header("Authorization", format!("Bearer {jwt}"))
        .header("X-Systemprompt-Credential-Broker", broker_secret)
        .header(MCP_SERVER_HEADER, server);
    if let Some(purpose) = caller_purpose {
        request = request.header(CALLER_HEADER, purpose);
    }
    let response = request.send().await.map_err(|e| {
        McpDomainError::transport(format!("token accessor request failed for '{server}'"), e)
    })?;

    match response.status() {
        reqwest::StatusCode::OK => {
            let body: AccessorResponse = response.json().await.map_err(|error| {
                tracing::warn!(server, %error, "token accessor returned an unreadable body");
                McpDomainError::ExternalAuthUnavailable {
                    server: server.to_owned(),
                    message: "token accessor returned an unreadable body".to_owned(),
                }
            })?;
            if body.access_token.trim().is_empty() {
                return Err(McpDomainError::ExternalAuthUnavailable {
                    server: server.to_owned(),
                    message: "token accessor returned an empty access_token".to_owned(),
                });
            }
            Ok(body.access_token)
        },
        reqwest::StatusCode::NOT_FOUND => Err(McpDomainError::ExternalAccountNotConnected {
            server: server.to_owned(),
        }),
        status => {
            let reason = response
                .json::<AccessorError>()
                .await
                .ok()
                .map(|body| body.error)
                .filter(|reason| !reason.trim().is_empty())
                .map_or_else(String::new, |reason| {
                    format!(
                        ": {}",
                        reason
                            .chars()
                            .take(ACCESSOR_ERROR_MAX_CHARS)
                            .collect::<String>()
                    )
                });
            tracing::warn!(
                server,
                status = status.as_u16(),
                reason = reason.trim_start_matches(": "),
                "token accessor refused the user's provider bearer"
            );
            Err(McpDomainError::ExternalAuthUnavailable {
                server: server.to_owned(),
                message: format!("token accessor returned status {status}{reason}"),
            })
        },
    }
}

pub fn outbound_headers<S: std::hash::BuildHasher>(
    ext: &ExternalAuth,
    bearer: &str,
    static_headers: &HashMap<String, String, S>,
    server: &str,
) -> McpDomainResult<HashMap<HeaderName, HeaderValue>> {
    let mut out = static_outbound_headers(static_headers, server)?;
    insert_header(&mut out, &ext.header, &ext.header_value(bearer), server)?;
    Ok(out)
}

pub fn static_outbound_headers<S: std::hash::BuildHasher>(
    static_headers: &HashMap<String, String, S>,
    server: &str,
) -> McpDomainResult<HashMap<HeaderName, HeaderValue>> {
    let mut out = HashMap::with_capacity(static_headers.len());
    for (name, value) in static_headers {
        insert_header(&mut out, name, value, server)?;
    }
    Ok(out)
}

fn insert_header(
    map: &mut HashMap<HeaderName, HeaderValue>,
    name: &str,
    value: &str,
    server: &str,
) -> McpDomainResult<()> {
    let header_name = HeaderName::try_from(name).map_err(|e| {
        McpDomainError::invalid_configuration(
            format!("external MCP server '{server}' has an invalid header name '{name}'"),
            e,
        )
    })?;
    let header_value = HeaderValue::try_from(value).map_err(|e| {
        McpDomainError::invalid_configuration(
            format!("external MCP server '{server}' has an invalid value for header '{name}'"),
            e,
        )
    })?;
    map.insert(header_name, header_value);
    Ok(())
}
