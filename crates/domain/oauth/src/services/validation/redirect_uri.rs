//! Redirect URI validation against registered clients.
//!
//! Matching is exact except for the RFC 8252 §7.3 loopback case: a registered
//! `http://127.0.0.1/cb` (or `localhost` / `[::1]`) matches the same path on
//! any port, because native clients bind an ephemeral port per run.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_models::AuthError;
use url::Url;

use super::registration_redirect::is_loopback;

pub fn validate_redirect_uri(
    registered_uris: &[String],
    requested_uri: Option<&str>,
) -> Result<String, AuthError> {
    let uri = requested_uri
        .filter(|u| !u.is_empty())
        .ok_or(AuthError::InvalidRedirectUri)?;

    if !registered_uris.contains(&uri.to_owned())
        && !matches_relative_uri(registered_uris, uri)
        && !matches_loopback_any_port(registered_uris, uri)
    {
        return Err(AuthError::InvalidRequest {
            reason: format!("Redirect URI '{uri}' not registered for this client"),
        });
    }

    Ok(uri.to_owned())
}

fn matches_relative_uri(registered_uris: &[String], requested_uri: &str) -> bool {
    if requested_uri.contains("://") {
        return false;
    }

    registered_uris.iter().any(|registered| {
        registered.starts_with('/') && !registered.starts_with("//") && registered == requested_uri
    })
}

fn matches_loopback_any_port(registered_uris: &[String], requested_uri: &str) -> bool {
    let Ok(requested) = Url::parse(requested_uri) else {
        return false;
    };
    if requested.scheme() != "http" || !is_loopback(&requested) {
        return false;
    }

    registered_uris.iter().any(|registered| {
        Url::parse(registered).is_ok_and(|reg| {
            reg.scheme() == "http"
                && is_loopback(&reg)
                && reg.host() == requested.host()
                && reg.path() == requested.path()
                && reg.query() == requested.query()
        })
    })
}
