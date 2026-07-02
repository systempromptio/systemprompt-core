//! Per-user bearer resolution for external MCP servers.
//!
//! An `external` MCP server backed by a third-party identity provider declares
//! an [`ExternalAuth`] block naming a relative accessor endpoint. The accessor
//! is served by an extension that banks the calling user's provider token; core
//! `GET`s it with the user's systemprompt JWT and receives a fresh bearer for
//! *that* user. The bearer (plus any static configured headers) is then
//! injected onto the outbound MCP request while the systemprompt credential is
//! withheld.

use std::collections::HashMap;

use http::{HeaderName, HeaderValue};
use systemprompt_models::mcp::ExternalAuth;
use systemprompt_models::{Config, RequestContext};

use super::validation::rewrite_url_for_internal_use;
use crate::error::{McpDomainError, McpDomainResult};

#[derive(serde::Deserialize)]
struct AccessorResponse {
    access_token: String,
}

pub(super) async fn resolve_external_bearer(
    ext: &ExternalAuth,
    context: &RequestContext,
    server: &str,
) -> McpDomainResult<String> {
    let jwt = context.auth_token();
    if jwt.as_str().is_empty() {
        return Err(McpDomainError::AuthRequired(format!(
            "external MCP server '{server}' requires an authenticated user to resolve its bearer"
        )));
    }

    let base = Config::get()?.api_external_url.clone();
    let accessor = accessor_url(&base, &ext.token_endpoint);
    fetch_external_bearer(&accessor, jwt.as_str(), server).await
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
    server: &str,
) -> McpDomainResult<String> {
    let response = reqwest::Client::new()
        .get(accessor)
        .header("Authorization", format!("Bearer {jwt}"))
        .send()
        .await
        .map_err(|e| {
            McpDomainError::Transport(format!("token accessor request failed for '{server}': {e}"))
        })?;

    match response.status() {
        reqwest::StatusCode::OK => {
            let body: AccessorResponse =
                response
                    .json()
                    .await
                    .map_err(|e| McpDomainError::ExternalAuthUnavailable {
                        server: server.to_owned(),
                        message: format!("token accessor returned an unreadable body: {e}"),
                    })?;
            if body.access_token.trim().is_empty() {
                return Err(McpDomainError::ExternalAuthUnavailable {
                    server: server.to_owned(),
                    message: "token accessor returned an empty access_token".to_owned(),
                });
            }
            Ok(body.access_token)
        },
        reqwest::StatusCode::NOT_FOUND => Err(McpDomainError::ExternalAuthUnavailable {
            server: server.to_owned(),
            message: "no token banked for this user; connect the provider account first".to_owned(),
        }),
        status => Err(McpDomainError::ExternalAuthUnavailable {
            server: server.to_owned(),
            message: format!("token accessor returned status {status}"),
        }),
    }
}

pub fn outbound_headers(
    ext: &ExternalAuth,
    bearer: &str,
    static_headers: &HashMap<String, String>,
    server: &str,
) -> McpDomainResult<HashMap<HeaderName, HeaderValue>> {
    let mut out = static_outbound_headers(static_headers, server)?;
    insert_header(&mut out, &ext.header, &ext.header_value(bearer), server)?;
    Ok(out)
}

pub fn static_outbound_headers(
    static_headers: &HashMap<String, String>,
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
        McpDomainError::Configuration(format!(
            "external MCP server '{server}' has an invalid header name '{name}': {e}"
        ))
    })?;
    let header_value = HeaderValue::try_from(value).map_err(|e| {
        McpDomainError::Configuration(format!(
            "external MCP server '{server}' has an invalid value for header '{name}': {e}"
        ))
    })?;
    map.insert(header_name, header_value);
    Ok(())
}
