//! The Drive sync's token view.
//!
//! [`DriveTokens`] is the narrow view of the deployment's token authority
//! that the bridge Drive routes hold. It does two things and nothing else:
//! trade a presented bridge device token for a Drive sync token, and verify a
//! presented Drive sync token. Every token it mints carries the Drive sync
//! audience alone and the `user` permission alone; it verifies no other
//! audience, so its holder can neither mint nor admit any other token kind.
//! The documents member builds it in its provide from the sealed token
//! authority and confers it on `gateway-http`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use chrono::{DateTime, Utc};
use systemprompt_identifiers::ClientId;

use crate::BoxedSource;
use crate::auth::AuthUser;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct DriveSyncGrant {
    pub token: String,
    pub expires_at: DateTime<Utc>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DriveSyncAttendance {
    Interactive,
    NonInteractive,
}

#[derive(Debug, Clone)]
pub struct DriveSyncPrincipal {
    pub user: AuthUser,
    pub client_id: Option<ClientId>,
    pub attendance: Option<DriveSyncAttendance>,
}

#[derive(Debug, thiserror::Error)]
pub enum DriveTokenError {
    #[error("the presented token does not verify for the Drive sync")]
    Unverified(#[source] BoxedSource),
    #[error("only the bridge device's own token can start a Drive sync")]
    NotDeviceToken,
    #[error("a purposed or delegated token cannot sync")]
    PurposedOrDelegated,
    #[error("the bridge token has expired")]
    Expired,
    #[error("a Drive sync needs a user account")]
    NoUserPermission,
    #[error("drive sync token was not minted")]
    Mint(#[source] BoxedSource),
}

/// Trades a bridge device token for a Drive sync token and verifies Drive
/// sync tokens; it mints and admits no other audience.
///
/// `trade` refuses anything but the bridge client's own unpurposed,
/// undelegated, unexpired device token held by an account with the `user`
/// permission; the grant expires no later than the device token. `verify`
/// admits only a Drive sync token without purpose or delegation chain.
/// Injected as `dyn DriveTokens`, hence `#[async_trait]`.
#[async_trait]
pub trait DriveTokens: Send + Sync + std::fmt::Debug {
    async fn trade(&self, device_token: &str) -> Result<DriveSyncGrant, DriveTokenError>;

    async fn verify(&self, drive_token: &str) -> Result<DriveSyncPrincipal, DriveTokenError>;
}

pub type DynDriveTokens = Arc<dyn DriveTokens>;
