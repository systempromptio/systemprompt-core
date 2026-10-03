//! The token-endpoint grant input, the issued-token outcome and the origin of
//! the request that asked for it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::net::IpAddr;

use http::HeaderMap;
use serde::{Deserialize, Serialize};

/// The RFC 6749 §4 / RFC 8693 / RFC 7523 token request, as form-decoded.
#[derive(Debug, Deserialize)]
pub struct TokenRequest {
    pub grant_type: String,
    pub code: Option<String>,
    pub redirect_uri: Option<String>,
    pub client_id: Option<String>,
    pub client_secret: Option<String>,
    pub refresh_token: Option<String>,
    pub scope: Option<String>,
    pub code_verifier: Option<String>,
    pub resource: Option<String>,
    pub plugin_id: Option<String>,
    pub audience: Option<String>,
    pub subject_token: Option<String>,
    pub subject_token_type: Option<String>,
    pub actor_token: Option<String>,
    pub actor_token_type: Option<String>,
    pub requested_token_type: Option<String>,
    pub assertion: Option<String>,
}

/// The RFC 6749 §5.1 successful token response.
#[derive(Debug, Serialize)]
pub struct TokenResponse {
    pub access_token: String,
    pub token_type: String,
    pub expires_in: i64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub refresh_token: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub scope: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub issued_token_type: Option<String>,
}

/// Request headers and caller address a minted session's analytics come from.
#[derive(Debug, Clone, Copy)]
pub struct RequestOrigin<'a> {
    pub headers: &'a HeaderMap,
    pub caller_ip: Option<IpAddr>,
}
