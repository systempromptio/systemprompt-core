//! Bridge exchange codes: a single-use, ten-minute code a person carries to
//! a device and swaps for bridge access or a device PAT. A service account
//! never receives one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{Duration as ChronoDuration, Utc};
use http::HeaderMap;
use rand::Rng;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use systemprompt_identifiers::UserId;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{AnalyticsProvider, SessionProvider, UserProvider};

use super::{BridgeAccessRequest, BridgeAuthResult, issue_bridge_access};
use crate::error::OauthResult as Result;
use crate::repository::{CreateExchangeCodeParams, OAuthRepository};
use crate::services::authenticated_user::load_interactive_user;

const EXCHANGE_CODE_BYTES: usize = 32;
const EXCHANGE_CODE_TTL_SECONDS: i64 = 600;

#[derive(Debug, Clone, Serialize)]
pub struct BridgeExchangeCode {
    pub code: String,
    pub expires_at: chrono::DateTime<Utc>,
}

pub async fn issue_bridge_exchange_code(
    repo: &OAuthRepository,
    users: &dyn UserProvider,
    user_id: &UserId,
) -> Result<BridgeExchangeCode> {
    load_interactive_user(users, user_id).await?;
    let code = generate_exchange_code();
    let code_hash = hash_exchange_code(&code);
    let expires_at = Utc::now() + ChronoDuration::seconds(EXCHANGE_CODE_TTL_SECONDS);

    repo.create_bridge_exchange_code(CreateExchangeCodeParams {
        code_hash: &code_hash,
        user_id,
        expires_at,
    })
    .await?;

    Ok(BridgeExchangeCode { code, expires_at })
}

/// One-shot exchange of a bridge session code presented by a caller.
#[derive(Debug, Clone, Copy)]
pub struct BridgeExchangeRequest<'a> {
    pub request_headers: &'a HeaderMap,
    pub caller_ip: Option<IpAddr>,
    pub code: &'a str,
    pub keys: &'a SigningKeys,
}

pub async fn exchange_bridge_session_code(
    repo: &OAuthRepository,
    analytics: &dyn AnalyticsProvider,
    sessions: &dyn SessionProvider,
    users: &dyn UserProvider,
    exchange: BridgeExchangeRequest<'_>,
) -> Result<Option<BridgeAuthResult>> {
    let BridgeExchangeRequest {
        request_headers,
        caller_ip,
        code,
        keys,
    } = exchange;
    let code_hash = hash_exchange_code(code);
    let Some(user_id) = repo.consume_bridge_exchange_code(&code_hash).await? else {
        return Ok(None);
    };
    load_interactive_user(users, &user_id).await?;
    let result = issue_bridge_access(
        keys,
        analytics,
        sessions,
        users,
        BridgeAccessRequest::bridge(request_headers, caller_ip, &user_id),
    )
    .await?;
    Ok(Some(result))
}

pub fn hash_exchange_code(code: &str) -> String {
    let digest = Sha256::digest(code.as_bytes());
    let mut out = String::with_capacity(64);
    for byte in digest {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}

fn generate_exchange_code() -> String {
    let mut raw = [0u8; EXCHANGE_CODE_BYTES];
    rand::rng().fill_bytes(&mut raw);
    let mut out = String::with_capacity(EXCHANGE_CODE_BYTES * 2);
    for byte in raw {
        out.push_str(&format!("{byte:02x}"));
    }
    out
}
