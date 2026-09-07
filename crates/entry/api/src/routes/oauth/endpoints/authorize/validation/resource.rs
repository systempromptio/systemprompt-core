//! RFC 8707 resource-indicator validation with the OAuth-strict SSRF gate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::net::OutboundUrlError;

use super::SelfOrigins;

pub(super) fn validate_resource_uri(
    resource: &str,
    self_origins: &SelfOrigins,
) -> Result<(), String> {
    let url = reqwest::Url::parse(resource)
        .map_err(|_e| format!("Invalid resource URI: '{resource}' is not a valid absolute URI"))?;

    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(format!(
            "Resource URI must use https or http scheme, got '{}'",
            url.scheme()
        ));
    }

    if url.fragment().is_some() {
        return Err("Resource URI must not contain a fragment".to_owned());
    }

    if self_origins.matches(&url.origin()) {
        return Ok(());
    }

    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let host_is_loopback_name = host == "localhost";
    let last_label = host.rsplit('.').next();
    let host_is_mdns_suffix = matches!(last_label, Some("local" | "internal"));
    if host_is_loopback_name || host_is_mdns_suffix {
        return Err(format!(
            "Resource URI host '{host}' is an internal or private network address"
        ));
    }
    if let Some(url::Host::Ipv4(ip)) = url.host()
        && ip.is_loopback()
    {
        return Err(format!(
            "Resource URI host '{ip}' is an internal or private (loopback) network address"
        ));
    }
    match systemprompt_models::net::validate_outbound_url(resource) {
        Ok(_) | Err(OutboundUrlError::NonLoopbackHttp) => Ok(()),
        Err(e @ OutboundUrlError::BlockedHost(_)) => Err(format!(
            "Resource URI points to an internal or private network address: {e}"
        )),
        Err(e) => Err(format!("Invalid resource URI: {e}")),
    }
}

pub(super) async fn resolve_resource_scopes(
    state: &systemprompt_oauth::OAuthState,
    resource: &str,
) -> Option<String> {
    let registry = state.mcp_registry()?;
    crate::routes::proxy::mcp::get_mcp_server_scopes_from_resource(registry.as_ref(), resource)
        .await
        .map(|scopes| scopes.join(" "))
}
