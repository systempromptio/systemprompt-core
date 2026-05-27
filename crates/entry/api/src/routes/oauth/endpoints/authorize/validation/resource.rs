pub(super) fn validate_resource_uri(resource: &str) -> Result<(), String> {
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

    // SSRF guard for OAuth resource indicators. The OAuth surface is stricter
    // than the workspace default `validate_outbound_url`: loopback hostnames
    // and `.internal` / `.local` suffixes are also rejected, because an OAuth
    // resource URI is presented by the relying party and must reference a
    // routable, externally-reachable service.
    let host = url.host_str().unwrap_or_default().to_ascii_lowercase();
    let host_is_loopback_name = host == "localhost";
    let host_is_mdns_suffix = host.ends_with(".local") || host.ends_with(".internal");
    if host_is_loopback_name || host_is_mdns_suffix {
        return Err(format!(
            "Resource URI host '{host}' is an internal or private network address"
        ));
    }
    if let Some(url::Host::Ipv4(ip)) = url.host() {
        if ip.is_loopback() {
            return Err(format!(
                "Resource URI host '{ip}' is an internal or private (loopback) network address"
            ));
        }
    }
    // Defer the broader private-range / link-local / blocked-IP check to the
    // workspace-canonical guard. The scheme gate is OAuth's own concern (we
    // accept http above for legacy relying parties) — only fail on the
    // address-block rule.
    use systemprompt_models::net::OutboundUrlError;
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
