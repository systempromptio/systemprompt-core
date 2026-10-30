//! The Drive sync credential exchange.
//!
//! A device trades its private bridge device token for a short token whose
//! only audience is [`DRIVE_SYNC_AUDIENCE`]. The caller decodes the device
//! token against [`device_audiences`]; [`DriveSyncTokenService::exchange`]
//! applies the rules: the bridge client's own token, no purpose, no delegation
//! chain, unexpired, held by an account with the `user` permission. The new
//! token carries the device token's user, session and `interaction` claim,
//! expires no later than it (and within [`MAX_TTL`]) and holds `user` alone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use chrono::{Duration, TimeZone, Utc};
use systemprompt_identifiers::{AccessTokenId, ClientId};
use systemprompt_models::auth::{
    AuthenticatedUser, JwtAudience, Permission, TokenAuthority, TokenError, UserJwtRequest,
};
use systemprompt_models::documents::{BRIDGE_DEVICE_AUDIENCE, DRIVE_SYNC_AUDIENCE, DriveSyncToken};
use systemprompt_security::JwtUserContext;
use systemprompt_traits::AuthUser;

pub const MAX_TTL: Duration = Duration::hours(1);

#[must_use]
pub fn device_audiences() -> [JwtAudience; 1] {
    [JwtAudience::Resource(BRIDGE_DEVICE_AUDIENCE.to_owned())]
}

#[must_use]
pub fn drive_audiences() -> [JwtAudience; 1] {
    [JwtAudience::Resource(DRIVE_SYNC_AUDIENCE.to_owned())]
}

/// Why a device token was refused or the Drive sync token not minted.
#[derive(Debug, thiserror::Error)]
pub enum DriveSyncError {
    #[error("only the bridge device's own token can start a Drive sync")]
    NotDeviceToken,
    #[error("the bridge token has expired")]
    Expired,
    #[error("a Drive sync needs a user account")]
    NoUserPermission,
    #[error("drive sync token was not minted")]
    Mint(#[source] TokenError),
}

/// Trades a decoded bridge device token for the Drive sync token.
#[derive(Debug, Clone)]
pub struct DriveSyncTokenService {
    tokens: Arc<dyn TokenAuthority>,
}

impl DriveSyncTokenService {
    #[must_use]
    pub fn new(tokens: Arc<dyn TokenAuthority>) -> Self {
        Self { tokens }
    }

    pub fn exchange(
        &self,
        device: &JwtUserContext,
        user: AuthUser,
    ) -> Result<DriveSyncToken, DriveSyncError> {
        let bridge_client = device.client_id.as_ref() == Some(&ClientId::bridge());
        if !bridge_client || device.purpose.is_some() || !device.act_chain.is_empty() {
            return Err(DriveSyncError::NotDeviceToken);
        }
        let bridge_expiry = Utc
            .timestamp_opt(device.exp, 0)
            .single()
            .unwrap_or_else(Utc::now);
        let ttl = (bridge_expiry - Utc::now()).min(MAX_TTL);
        if ttl <= Duration::zero() {
            return Err(DriveSyncError::Expired);
        }
        let who = AuthenticatedUser::from_provider_user(user);
        if !who.has_permission(Permission::User) {
            return Err(DriveSyncError::NoUserPermission);
        }
        let request = UserJwtRequest {
            permissions: vec![Permission::User],
            user: who,
            session_id: device.session_id.clone(),
            jti: AccessTokenId::generate(),
            audience: drive_audiences().to_vec(),
            ttl,
            client_id: Some(ClientId::bridge()),
            purpose: None,
            plugin_id: None,
            interaction: device.interaction,
            browser_session: false,
        };
        let token = self
            .tokens
            .mint_user_jwt(&request)
            .map_err(DriveSyncError::Mint)?;
        Ok(DriveSyncToken {
            token,
            expires_at: Utc::now() + ttl,
        })
    }
}
