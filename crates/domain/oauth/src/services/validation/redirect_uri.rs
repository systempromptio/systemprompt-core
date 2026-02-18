use systemprompt_models::AuthError;

pub fn validate_redirect_uri(
    registered_uris: &[String],
    requested_uri: Option<&str>,
) -> Result<String, AuthError> {
    let uri = requested_uri
        .filter(|u| !u.is_empty())
        .ok_or(AuthError::InvalidRedirectUri)?;

    if !registered_uris.contains(&uri.to_string()) && !matches_relative_uri(registered_uris, uri) {
        return Err(AuthError::InvalidRequest {
            reason: format!("Redirect URI '{uri}' not registered for this client"),
        });
    }

    Ok(uri.to_string())
}

/// Check if any registered relative path URI matches the path of the requested
/// absolute URI. A registered URI like `/admin/login` will match `https://example.com/admin/login`.
/// Only paths starting with `/` (and not `//`) are treated as relative.
fn matches_relative_uri(registered_uris: &[String], requested_uri: &str) -> bool {
    let requested_path = match requested_uri.find("://") {
        Some(scheme_end) => {
            let after_scheme = &requested_uri[scheme_end + 3..];
            after_scheme
                .find('/')
                .map_or("/", |slash_pos| &after_scheme[slash_pos..])
        },
        None => return false,
    };

    registered_uris.iter().any(|registered| {
        registered.starts_with('/') && !registered.starts_with("//") && registered == requested_path
    })
}
