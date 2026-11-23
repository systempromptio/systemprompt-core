//! Shared state for A2A OAuth authentication.
//!
//! [`AgentOAuthState`] bundles the auth config, the audiences an A2A request
//! may carry, the database handle and the kernel's request authenticator the
//! auth middleware and the OAuth scope check verify every token through.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use systemprompt_database::Database;
pub use systemprompt_manifest::AgentOAuthConfig;
use systemprompt_models::auth::{DynRequestAuthenticator, JwtAudience};

#[derive(Clone)]
pub struct AgentOAuthState {
    pub config: AgentOAuthConfig,
    pub audiences: Arc<[JwtAudience]>,
    pub db: Arc<Database>,
    pub authenticator: Option<DynRequestAuthenticator>,
}

impl std::fmt::Debug for AgentOAuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentOAuthState")
            .field("config", &self.config)
            .field("audiences", &self.audiences)
            .field("db", &"<Database>")
            .field("authenticator", &self.authenticator.is_some())
            .finish()
    }
}

impl AgentOAuthState {
    pub fn new(db: Arc<Database>, config: AgentOAuthConfig, audiences: Vec<JwtAudience>) -> Self {
        Self {
            config,
            audiences: Arc::from(audiences),
            db,
            authenticator: None,
        }
    }

    #[must_use]
    pub fn with_authenticator(mut self, authenticator: DynRequestAuthenticator) -> Self {
        self.authenticator = Some(authenticator);
        self
    }
}
