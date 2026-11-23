//! Bridge exchange codes: a single-use, ten-minute code a person carries to
//! a device and swaps for bridge access or a device PAT. A service account
//! never receives one.
//!
//! A device link may also carry a person's UI session. The approving page
//! passes the `person_ui` client's PKCE request and the browser's sign-in
//! time to [`issue_device_link_code`], which stores them on the code only
//! after checking the client, its redirect, the S256 challenge and the scope.
//! The device redeems that code for its key and an authorization code; the
//! verifier never leaves the client, so the device key alone opens no UI
//! session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Duration as ChronoDuration, Utc};
use http::HeaderMap;
use rand::Rng;
use serde::Serialize;
use sha2::{Digest, Sha256};
use std::net::IpAddr;
use systemprompt_identifiers::{ClientId, UserId};
use systemprompt_manifest::Config;
use systemprompt_security::keys::SigningKeys;
use systemprompt_traits::{AnalyticsProvider, SessionProvider, UserProvider};

use super::{BridgeAccessRequest, BridgeAuthResult, issue_bridge_access};
use crate::error::{OauthError, OauthResult as Result};
use crate::models::PkceMethod;
use crate::repository::{CreateExchangeCodeParams, DeviceLinkUiGrant, OAuthRepository};
use crate::services::authenticated_user::load_interactive_user;
use crate::services::validation::validate_redirect_uri;

const EXCHANGE_CODE_BYTES: usize = 32;
const EXCHANGE_CODE_TTL_SECONDS: i64 = 600;

#[derive(Debug, Clone, Serialize)]
pub struct BridgeExchangeCode {
    pub code: String,
    pub expires_at: DateTime<Utc>,
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

/// The UI session a device link asks for, as the approving page received it
/// from the client. `authenticated_at` is when the person signed in to the
/// approving browser, never the time of the approval.
#[derive(Debug, Clone, Copy)]
pub struct DeviceLinkUiRequest<'a> {
    pub client_id: &'a ClientId,
    pub redirect_uri: &'a str,
    pub scope: &'a str,
    pub code_challenge: &'a str,
    pub code_challenge_method: &'a str,
    pub authenticated_at: DateTime<Utc>,
}

pub async fn issue_device_link_code(
    repo: &OAuthRepository,
    users: &dyn UserProvider,
    user_id: &UserId,
    ui: DeviceLinkUiRequest<'_>,
) -> Result<BridgeExchangeCode> {
    load_interactive_user(users, user_id).await?;
    let redirect_uri = check_ui_request(repo, &ui).await?;
    let code = generate_exchange_code();
    let code_hash = hash_exchange_code(&code);
    let expires_at = Utc::now() + ChronoDuration::seconds(EXCHANGE_CODE_TTL_SECONDS);

    repo.create_device_link_code(
        CreateExchangeCodeParams {
            code_hash: &code_hash,
            user_id,
            expires_at,
        },
        DeviceLinkUiGrant {
            client_id: ui.client_id,
            redirect_uri: &redirect_uri,
            scope: ui.scope,
            code_challenge: ui.code_challenge,
            authenticated_at: ui.authenticated_at.min(Utc::now()),
        },
    )
    .await?;

    Ok(BridgeExchangeCode { code, expires_at })
}

async fn check_ui_request(repo: &OAuthRepository, ui: &DeviceLinkUiRequest<'_>) -> Result<String> {
    let refuse = |reason: &str| OauthError::Validation(reason.to_owned());
    if !repo.is_person_ui_client(ui.client_id).await? {
        return Err(refuse("client is not a declared person_ui client"));
    }
    let client = repo
        .find_client_by_id(ui.client_id)
        .await?
        .ok_or_else(|| refuse("client is not a declared person_ui client"))?;
    let redirect_uri = validate_redirect_uri(&client.redirect_uris, Some(ui.redirect_uri))
        .map_err(|_unregistered| refuse("redirect_uri is not registered for this client"))?;
    if ui.code_challenge.is_empty() || ui.code_challenge_method != PkceMethod::S256.as_str() {
        return Err(refuse("an S256 PKCE code_challenge is required"));
    }
    let scopes = OAuthRepository::parse_scopes(ui.scope);
    if scopes.is_empty() || scopes.iter().any(|scope| !client.scopes.contains(scope)) {
        return Err(refuse("scope is not one the client declares"));
    }
    Ok(redirect_uri)
}

/// One-shot exchange of a bridge session code presented by a caller.
#[derive(Debug, Clone, Copy)]
pub struct BridgeExchangeRequest<'a> {
    pub request_headers: &'a HeaderMap,
    pub caller_ip: Option<IpAddr>,
    pub code: &'a str,
    pub keys: &'a SigningKeys,
    pub config: &'a Config,
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
        config,
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
        BridgeAccessRequest::bridge(config, request_headers, caller_ip, &user_id).interactive(),
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
