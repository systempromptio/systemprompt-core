pub(super) fn validate_resource_uri(resource: &str) -> Result<(), String> {
    let url = reqwest::Url::parse(resource)
        .map_err(|_| format!("Invalid resource URI: '{resource}' is not a valid absolute URI"))?;

    if url.scheme() != "https" && url.scheme() != "http" {
        return Err(format!(
            "Resource URI must use https or http scheme, got '{}'",
            url.scheme()
        ));
    }

    if url.fragment().is_some() {
        return Err("Resource URI must not contain a fragment".to_string());
    }

    if let Some(host) = url.host_str() {
        if is_forbidden_host(host) {
            return Err(
                "Resource URI must not target internal or private network addresses".to_string(),
            );
        }
    }

    Ok(())
}

fn is_forbidden_host(host: &str) -> bool {
    let lower = host.to_lowercase();

    if lower == "localhost" || lower == "127.0.0.1" || lower == "::1" || lower == "0.0.0.0" {
        return true;
    }

    if lower.ends_with(".internal")
        || std::path::Path::new(&lower)
            .extension()
            .is_some_and(|ext| ext.eq_ignore_ascii_case("local"))
    {
        return true;
    }

    if lower.starts_with("10.") || lower.starts_with("192.168.") || lower.starts_with("169.254.") {
        return true;
    }

    if lower.starts_with("172.") {
        if let Some(second_octet_str) = lower
            .strip_prefix("172.")
            .and_then(|rest| rest.split('.').next())
        {
            if let Ok(second_octet) = second_octet_str.parse::<u8>() {
                if (16..=31).contains(&second_octet) {
                    return true;
                }
            }
        }
    }

    false
}

pub(super) async fn resolve_resource_scopes(resource: &str) -> Option<String> {
    crate::routes::proxy::mcp::get_mcp_server_scopes_from_resource(resource)
        .await
        .map(|scopes| scopes.join(" "))
}
