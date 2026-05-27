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

    Ok(())
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
