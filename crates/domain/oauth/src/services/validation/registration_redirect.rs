//! Redirect URI policy applied when a client registers (RFC 7591) or updates
//! its metadata (RFC 7592).
//!
//! Runtime matching stays exact ([`super::redirect_uri`]); this module decides
//! what may be registered in the first place. `web` clients get `https://`
//! or a loopback `http://`; `native` clients additionally get a private-use
//! scheme (RFC 8252 §7.1). Fragments and script-capable schemes are refused
//! for every client type. `client_uri` and `logo_uri` are shown to the user
//! on the consent page, so they must be plain `http(s)` URLs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use url::{Host, Url};

use crate::error::{OauthError, OauthResult};

const REFUSED_SCHEMES: &[&str] = &["javascript", "data", "file", "blob", "vbscript"];

pub fn validate_registration_redirect_uris(
    application_type: &str,
    redirect_uris: &[String],
) -> OauthResult<()> {
    for uri in redirect_uris {
        validate_one(application_type, uri)?;
    }
    Ok(())
}

fn validate_one(application_type: &str, uri: &str) -> OauthResult<()> {
    let parsed = Url::parse(uri).map_err(|e| {
        OauthError::Validation(format!("redirect_uri {uri:?} is not an absolute URL: {e}"))
    })?;

    if parsed.fragment().is_some() {
        return Err(OauthError::Validation(format!(
            "redirect_uri {uri:?} must not contain a fragment"
        )));
    }

    let scheme = parsed.scheme();
    if REFUSED_SCHEMES.contains(&scheme) {
        return Err(OauthError::Validation(format!(
            "redirect_uri {uri:?} uses a refused scheme"
        )));
    }

    match scheme {
        "https" => Ok(()),
        "http" if is_loopback(&parsed) => Ok(()),
        "http" => Err(OauthError::Validation(format!(
            "redirect_uri {uri:?} must use https unless it targets the loopback interface"
        ))),
        _ if application_type == "native" => Ok(()),
        _ => Err(OauthError::Validation(format!(
            "redirect_uri {uri:?} uses a private-use scheme, which requires application_type \
             \"native\""
        ))),
    }
}

pub fn validate_client_metadata_uri(field: &str, value: Option<&str>) -> OauthResult<()> {
    let Some(value) = value else {
        return Ok(());
    };
    let parsed = Url::parse(value)
        .map_err(|e| OauthError::Validation(format!("{field} {value:?} is not a URL: {e}")))?;
    if !matches!(parsed.scheme(), "http" | "https") {
        return Err(OauthError::Validation(format!(
            "{field} {value:?} must be an http(s) URL"
        )));
    }
    Ok(())
}

pub fn is_loopback(url: &Url) -> bool {
    match url.host() {
        Some(Host::Domain(domain)) => domain.eq_ignore_ascii_case("localhost"),
        Some(Host::Ipv4(ip)) => ip.is_loopback(),
        Some(Host::Ipv6(ip)) => ip.is_loopback(),
        None => false,
    }
}
