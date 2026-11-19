//! The rules a `person_ui` client declaration must meet: a public client
//! limited to the `authorization_code` and `refresh_token` grants whose
//! redirects are loopback or private-use-scheme (RFC 8252), never a platform
//! client.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::ClientId;

use super::declaration::{DeclarationError, DeclaredClient, sorted_unique};

const PERSON_UI_GRANT_TYPES: &[&str] = &["authorization_code", "refresh_token"];

pub(super) fn validate(client: &DeclaredClient, id: &str) -> Result<(), DeclarationError> {
    let platform = [ClientId::web(), ClientId::bridge(), ClientId::cli()]
        .iter()
        .any(|platform| platform.as_str() == id);
    if platform || id.starts_with("sys_") || id.starts_with("https://") {
        return Err(DeclarationError::new(format!(
            "oauth client {id}: person_ui is for an operator's own UI client, not a platform \
             client"
        )));
    }
    if client.secret_key.is_some() {
        return Err(DeclarationError::new(format!(
            "oauth client {id}: a person_ui client is public and takes no secret_key"
        )));
    }
    let grants = sorted_unique(&client.grant_types);
    if !grants.iter().any(|g| g == "authorization_code")
        || grants
            .iter()
            .any(|g| !PERSON_UI_GRANT_TYPES.contains(&g.as_str()))
    {
        return Err(DeclarationError::new(format!(
            "oauth client {id}: a person_ui client declares authorization_code and at most \
             refresh_token"
        )));
    }
    if client.redirect_uris.is_empty() {
        return Err(DeclarationError::new(format!(
            "oauth client {id}: a person_ui client needs a redirect URI"
        )));
    }
    if let Some(bad) = client
        .redirect_uris
        .iter()
        .find(|uri| !is_native_redirect(uri.trim()))
    {
        return Err(DeclarationError::new(format!(
            "oauth client {id}: person_ui redirect '{bad}' is neither a loopback http URI nor \
             a private-use scheme"
        )));
    }
    Ok(())
}

// Why: RFC 8252 §7.1 and §7.3 — a native app receives its redirect on a
// reverse-DNS private-use scheme or on the IP loopback literal; `localhost` is
// not a loopback literal (§8.3) and a fragment never reaches the redirect.
fn is_native_redirect(uri: &str) -> bool {
    let Ok(url) = url::Url::parse(uri) else {
        return false;
    };
    if url.fragment().is_some() {
        return false;
    }
    match url.scheme() {
        "http" => {
            matches!(
                url.host(),
                Some(url::Host::Ipv4(ip)) if ip.is_loopback()
            ) || matches!(url.host(), Some(url::Host::Ipv6(ip)) if ip.is_loopback())
        },
        "https" => false,
        scheme => scheme.contains('.'),
    }
}
