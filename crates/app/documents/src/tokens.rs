//! The two narrow token views the documents member builds from the sealed
//! token authority, one type per holder.
//!
//! [`DocumentLinkTokens`](crate::tokens::DocumentLinkTokens) is conferred on
//! `files-http`. It signs a download link (audience `documents.download`) or an
//! upload link (audience `documents.upload`), verifies a presented token
//! against exactly one of those two audiences, and verifies the MCP hop token a
//! managed server presents when it asks for a link. It mints no other audience
//! and verifies no user, session or UI token.
//!
//! [`DocumentDriveTokens`](crate::tokens::DocumentDriveTokens) is the
//! [`DriveTokens`](systemprompt_traits::DriveTokens) view conferred on
//! `gateway-http`: it trades a bridge device token for a Drive sync token
//! and verifies Drive sync tokens, and nothing else.
//!
//! A host that seeds no token authority (a child host holds verification keys
//! only), or composes no request authenticator, gets views that refuse every
//! call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{JwtToken, McpServerId};
use systemprompt_models::auth::{
    DynRequestAuthenticator, JwtAudience, JwtClaims, McpHopClaims, TokenAuthority, TokenError,
    TokenInteraction, TokenResult, UserJwtRequest,
};
use systemprompt_oauth_issuance::drive_sync::{
    DriveSyncError, DriveSyncTokenService, device_audiences, drive_audiences,
};
use systemprompt_traits::{
    DriveSyncAttendance, DriveSyncGrant, DriveSyncPrincipal, DriveTokenError, DriveTokens,
};

use crate::download::DOWNLOAD_AUDIENCE;
use crate::upload_links::UPLOAD_AUDIENCE;

#[derive(Debug, thiserror::Error)]
enum ViewRefusal {
    #[error("this host holds no token authority")]
    NoAuthority,
    #[error("this host holds no request authenticator")]
    NoAuthenticator,
    #[error("the link view signs only download and upload links")]
    NotALink,
}

fn refused(reason: ViewRefusal) -> TokenError {
    TokenError::Rejected(Box::new(reason))
}

fn link_audience(name: &str) -> [JwtAudience; 1] {
    [JwtAudience::Resource(name.to_owned())]
}

/// Signs and verifies document download and upload links; see the module
/// docs.
#[derive(Debug, Clone)]
pub struct DocumentLinkTokens {
    authority: Option<Arc<dyn TokenAuthority>>,
}

impl DocumentLinkTokens {
    #[must_use]
    pub const fn new(authority: Option<Arc<dyn TokenAuthority>>) -> Self {
        Self { authority }
    }

    fn authority(&self) -> TokenResult<&dyn TokenAuthority> {
        self.authority
            .as_deref()
            .ok_or_else(|| refused(ViewRefusal::NoAuthority))
    }

    pub(crate) fn mint_link(&self, request: &UserJwtRequest) -> TokenResult<String> {
        let is_link = [DOWNLOAD_AUDIENCE, UPLOAD_AUDIENCE]
            .iter()
            .any(|audience| request.audience == link_audience(audience));
        if !is_link
            || !request.permissions.is_empty()
            || request.client_id.is_some()
            || request.browser_session
            || request.ui_session
        {
            return Err(refused(ViewRefusal::NotALink));
        }
        self.authority()?.mint_user_jwt(request)
    }

    pub fn verify_download(&self, token: &str) -> TokenResult<JwtClaims> {
        self.authority()?
            .verify_bearer(token, &link_audience(DOWNLOAD_AUDIENCE))
    }

    pub fn verify_upload(&self, token: &str) -> TokenResult<JwtClaims> {
        self.authority()?
            .verify_bearer(token, &link_audience(UPLOAD_AUDIENCE))
    }

    pub fn verify_link_hop(&self, hop: &str, server: &McpServerId) -> TokenResult<McpHopClaims> {
        self.authority()?.verify_mcp_hop(hop, server)
    }
}

/// The [`DriveTokens`] view over the token authority and the request
/// authenticator; see the module docs.
#[derive(Debug, Clone)]
pub struct DocumentDriveTokens {
    exchange: Option<DriveSyncTokenService>,
    authenticator: Option<DynRequestAuthenticator>,
}

impl DocumentDriveTokens {
    #[must_use]
    pub fn new(
        authority: Option<Arc<dyn TokenAuthority>>,
        authenticator: Option<DynRequestAuthenticator>,
    ) -> Self {
        Self {
            exchange: authority.map(DriveSyncTokenService::new),
            authenticator,
        }
    }
}

impl DocumentDriveTokens {
    fn authenticator(&self) -> Result<&DynRequestAuthenticator, DriveTokenError> {
        self.authenticator
            .as_ref()
            .ok_or_else(|| unverified(ViewRefusal::NoAuthenticator))
    }
}

fn unverified(error: impl std::error::Error + Send + Sync + 'static) -> DriveTokenError {
    DriveTokenError::Unverified(Box::new(error))
}

const fn attendance(interaction: Option<TokenInteraction>) -> Option<DriveSyncAttendance> {
    match interaction {
        Some(TokenInteraction::Interactive) => Some(DriveSyncAttendance::Interactive),
        Some(TokenInteraction::NonInteractive) => Some(DriveSyncAttendance::NonInteractive),
        None => None,
    }
}

#[async_trait]
impl DriveTokens for DocumentDriveTokens {
    async fn trade(&self, device_token: &str) -> Result<DriveSyncGrant, DriveTokenError> {
        let exchange = self
            .exchange
            .as_ref()
            .ok_or_else(|| DriveTokenError::Mint(Box::new(ViewRefusal::NoAuthority)))?;
        let (claims, user) = self
            .authenticator()?
            .decode_for_audiences(&JwtToken::new(device_token), &device_audiences())
            .await
            .map_err(unverified)?;
        let token = exchange
            .exchange(&claims, user)
            .map_err(|error| match error {
                DriveSyncError::NotDeviceToken => DriveTokenError::NotDeviceToken,
                DriveSyncError::Expired => DriveTokenError::Expired,
                DriveSyncError::NoUserPermission => DriveTokenError::NoUserPermission,
                DriveSyncError::Mint(source) => DriveTokenError::Mint(Box::new(source)),
            })?;
        Ok(DriveSyncGrant {
            token: token.token,
            expires_at: token.expires_at,
        })
    }

    async fn verify(&self, drive_token: &str) -> Result<DriveSyncPrincipal, DriveTokenError> {
        let (claims, user) = self
            .authenticator()?
            .decode_for_audiences(&JwtToken::new(drive_token), &drive_audiences())
            .await
            .map_err(unverified)?;
        if claims.purpose.is_some() || !claims.act_chain.is_empty() {
            return Err(DriveTokenError::PurposedOrDelegated);
        }
        Ok(DriveSyncPrincipal {
            user,
            client_id: claims.client_id,
            attendance: attendance(claims.interaction),
        })
    }
}
