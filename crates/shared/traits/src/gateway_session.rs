//! The gateway's session mint.
//!
//! The gateway mints, or binds, a `user_sessions` row for an API-key caller.
//! Every method takes the presented API key, never a user id: the mint
//! verifies the key itself and mints, binds or attests only for that key's
//! owner, so its holder cannot create or adopt a session for any other user.
//! The `users` member provides it sealed and confers it on `gateway-http`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_identifiers::{ApiKeyId, SessionId, UserId};

use crate::BoxedSource;
use crate::analytics::SessionAnalytics;

#[derive(Debug, Clone, PartialEq, Eq)]
pub struct MintedGatewaySession {
    pub api_key_id: ApiKeyId,
    pub user_id: UserId,
    pub session_id: SessionId,
    pub presented_session_kept: bool,
}

#[derive(Debug, thiserror::Error)]
pub enum GatewaySessionMintError {
    #[error("gateway session mint failed: {context}")]
    Store {
        context: &'static str,
        #[source]
        source: BoxedSource,
    },
}

/// Mints and binds sessions for the verified owner of a presented API key.
///
/// `mint_for_api_key` mints a fresh session. `bind_for_api_key` keeps the
/// presented session when it is active and not another user's, else returns
/// the session bound to the key, minting and binding one when none is usable.
/// Both return `None` when the key does not verify. Injected as
/// `dyn GatewaySessionMint`, hence `#[async_trait]`.
#[async_trait]
pub trait GatewaySessionMint: Send + Sync {
    async fn mint_for_api_key(
        &self,
        presented: &str,
        analytics: &SessionAnalytics,
    ) -> Result<Option<MintedGatewaySession>, GatewaySessionMintError>;

    async fn bind_for_api_key(
        &self,
        presented: &str,
        presented_session: Option<&SessionId>,
        analytics: &SessionAnalytics,
    ) -> Result<Option<MintedGatewaySession>, GatewaySessionMintError>;
}

pub type DynGatewaySessionMint = Arc<dyn GatewaySessionMint>;
