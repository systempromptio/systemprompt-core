//! The caller credential a gateway request forwards upstream.
//!
//! A request that names its systemprompt credential in the dedicated
//! `x-systemprompt-authorization` header keeps `Authorization` for the
//! upstream: it is never read as identity, and it is the only credential a
//! caller-credential hop presents ([`caller_auth`],
//! [`presented_credential`]). The systemprompt credential itself is read by
//! [`systemprompt_security::extraction::extract_credential`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::http::HeaderMap;
use systemprompt_ai::PresentedCredential;
use systemprompt_identifiers::headers::{AUTHORIZATION, SYSTEMPROMPT_AUTHORIZATION};
use systemprompt_security::credential::{AuthHeader, AuthScheme};

const BEARER_PREFIX: &str = "Bearer ";

pub fn caller_auth(headers: &HeaderMap) -> Option<AuthHeader> {
    headers.get(SYSTEMPROMPT_AUTHORIZATION)?;
    let token = headers
        .get(AUTHORIZATION)?
        .to_str()
        .ok()?
        .strip_prefix(BEARER_PREFIX)?
        .trim();
    (!token.is_empty()).then(|| AuthHeader {
        scheme: AuthScheme::Bearer,
        value: token.to_owned(),
    })
}

pub fn presented_credential(headers: &HeaderMap) -> Option<PresentedCredential> {
    let auth = caller_auth(headers)?;
    let presented = headers
        .iter()
        .filter(|(name, _)| systemprompt_wire::upstream::is_presented_request_header(name.as_str()))
        .filter_map(|(name, value)| {
            Some((name.as_str().to_owned(), value.to_str().ok()?.to_owned()))
        })
        .collect();
    Some(PresentedCredential {
        auth,
        headers: presented,
    })
}
