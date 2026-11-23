//! `POST /v1/auth/bridge/session-pat`: redeems a device-link exchange code.
//!
//! The reply carries the device's durable key and, when the link was approved
//! for a `person_ui` client, an authorization code for that person's UI
//! session. The authorization code is bound to the PKCE challenge the client
//! sent to the approving page and to the device key minted here. The client
//! redeems it at the token endpoint with its verifier, so the device key alone
//! opens no UI session, and the session's refresh is refused once the key is
//! revoked. A device key whose UI code cannot be minted is revoked and the
//! request fails; no key is handed out without the session the link asked for.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Json;
use serde::Deserialize;
use systemprompt_identifiers::{ApiKeyId, UserId};
use systemprompt_models::bridge::gateway::DevicePatResponse;
use systemprompt_oauth::repository::{MintAuthCodeParams, OAuthRepository, StoredUiGrant};
use systemprompt_oauth::services::{hash_exchange_code, refuse_service_account};
use systemprompt_runtime::AppContext;
use systemprompt_users::{
    ApiKeyKind, ApiKeyLifetime, ApiKeyLimits, ApiKeyService, IssueApiKeyParams,
};

use super::auth::require_user_provider;
use crate::error::ApiHttpError;

const DEFAULT_DEVICE_NAME: &str = "bridge device-link";
const S256: &str = "S256";

#[derive(Debug, Deserialize)]
pub struct SessionPatBody {
    pub code: String,
    #[serde(default)]
    pub device_name: Option<String>,
}

pub async fn session_pat(
    ctx: AppContext,
    Json(body): Json<SessionPatBody>,
) -> Result<Json<DevicePatResponse>, ApiHttpError> {
    let code = body.code.trim();
    if code.is_empty() {
        return Err(ApiHttpError::bad_request("missing exchange code"));
    }
    let device_name = body
        .device_name
        .as_deref()
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .unwrap_or(DEFAULT_DEVICE_NAME);

    let repo = &ctx.oauth_repositories().oauth;
    let link = repo
        .consume_device_link_code(&hash_exchange_code(code))
        .await?
        .ok_or_else(|| {
            ApiHttpError::unauthorized("exchange code invalid, expired, or already consumed")
        })?;
    let users = require_user_provider(&ctx)?;
    let user = users
        .find_by_id(&link.user_id)
        .await
        .map_err(|error| {
            tracing::warn!(%error, "user lookup for the session PAT failed");
            ApiHttpError::internal("user lookup failed", error)
        })?
        .ok_or_else(|| ApiHttpError::unauthorized("exchange code user no longer exists"))?;
    refuse_service_account(user.is_service_account, &link.user_id)?;

    let service = ApiKeyService::new(
        Arc::clone(ctx.user_repository()),
        ApiKeyLifetime::from_max_days(ctx.config().api_key_max_lifetime_days),
    );
    let issued = service
        .issue(IssueApiKeyParams {
            user_id: &link.user_id,
            name: device_name,
            expires_at: None,
            limits: &ApiKeyLimits::default(),
            scopes: &[],
            purpose: None,
            kind: ApiKeyKind::Device,
        })
        .await?;

    let ui_code = match &link.ui_grant {
        None => None,
        Some(grant) => match mint_ui_code(repo, &link.user_id, grant, &issued.record.id).await {
            Ok(code) => Some(code),
            Err(error) => {
                if let Err(undo) = service.revoke(&issued.record.id, &link.user_id).await {
                    tracing::error!(
                        api_key_id = %issued.record.id,
                        error = %undo,
                        "A device key whose UI code failed could not be revoked"
                    );
                }
                return Err(error);
            },
        },
    };

    Ok(Json(DevicePatResponse {
        pat: issued.secret,
        ui_code,
    }))
}

async fn mint_ui_code(
    repo: &OAuthRepository,
    user_id: &UserId,
    grant: &StoredUiGrant,
    device_key: &ApiKeyId,
) -> Result<String, ApiHttpError> {
    let code = repo
        .mint_device_authorization_code(
            MintAuthCodeParams {
                client_id: &grant.client_id,
                user_id,
                redirect_uri: &grant.redirect_uri,
                scope: Some(&grant.scope),
                code_challenge: &grant.code_challenge,
                code_challenge_method: S256,
                resource: None,
                authenticated_at: grant.authenticated_at,
            },
            device_key,
        )
        .await?;
    Ok(code.as_str().to_owned())
}
