//! What every `gateway-http` handler reads.
//!
//! [`GatewayHttpState`] is built once per mount by the surface from the
//! route-scoped resolution: the request authenticator (the one verifier; no
//! handler decodes a token itself), the session mint and engines conferred
//! on `gateway-http`, the gateway repositories, and the host handles the API
//! mount passes. [`GatewayHttpRepositories`] is what the member builds in its
//! own provide and provides sealed, so only its own route state reads it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_database::DbPool;
use systemprompt_gateway::{GatewayEngines, GatewayHostHandles, GatewayRepositories};
use systemprompt_models::auth::DynRequestAuthenticator;
use systemprompt_traits::{DynContextMaterializer, DynGatewaySessionMint};
use systemprompt_users::{ApiKeyService, UserApiKey, UserResult};

use crate::http::bridge::BridgeRepositories;

/// The state shared by every `gateway-http` handler.
#[derive(Clone)]
pub struct GatewayHttpState {
    pub authenticator: DynRequestAuthenticator,
    pub api_keys: GatewayApiKeys,
    pub session_mint: DynGatewaySessionMint,
    pub repos: Arc<GatewayRepositories>,
    pub engines: Arc<GatewayEngines>,
    pub host: GatewayHostHandles,
    pub db: DbPool,
}

impl std::fmt::Debug for GatewayHttpState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayHttpState").finish_non_exhaustive()
    }
}

/// What the `gateway-http` member builds in its provide: the API-key check,
/// the context materializer its repositories bind conversations with, and the
/// bridge routes' repositories.
#[derive(Clone)]
pub struct GatewayHttpRepositories {
    pub api_keys: GatewayApiKeys,
    pub contexts: DynContextMaterializer,
    pub bridge: BridgeRepositories,
}

impl std::fmt::Debug for GatewayHttpRepositories {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayHttpRepositories")
            .finish_non_exhaustive()
    }
}

/// The gateway's API-key check: it verifies a presented key and does
/// nothing else — it issues, lists and revokes no key.
#[derive(Clone)]
pub struct GatewayApiKeys(Arc<ApiKeyService>);

impl GatewayApiKeys {
    #[must_use]
    pub fn new(service: ApiKeyService) -> Self {
        Self(Arc::new(service))
    }

    pub async fn verify(&self, presented: &str) -> UserResult<Option<UserApiKey>> {
        self.0.verify(presented).await
    }
}

impl std::fmt::Debug for GatewayApiKeys {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayApiKeys").finish_non_exhaustive()
    }
}
