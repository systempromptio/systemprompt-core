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

fn matches_relative_uri(registered_uris: &[String], requested_uri: &str) -> bool {
    if requested_uri.contains("://") {
        return false;
    }

    registered_uris.iter().any(|registered| {
        registered.starts_with('/') && !registered.starts_with("//") && registered == requested_uri
    })
}
