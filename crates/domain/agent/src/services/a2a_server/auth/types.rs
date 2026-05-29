//! Shared state for A2A OAuth authentication.
//!
//! [`AgentOAuthState`] bundles the auth config, validation service, database
//! handle, and optional JWT/user providers carried through the auth middleware.

use std::sync::Arc;
use systemprompt_database::Database;
pub use systemprompt_models::AgentOAuthConfig;
use systemprompt_models::auth::JwtAudience;
use systemprompt_security::AuthValidationService;
use systemprompt_traits::{DynJwtValidationProvider, DynUserProvider};

#[derive(Clone)]
pub struct AgentOAuthState {
    pub config: AgentOAuthConfig,
    pub auth_service: Arc<AuthValidationService>,
    pub db: Arc<Database>,
    pub jwt_provider: Option<DynJwtValidationProvider>,
    pub user_provider: Option<DynUserProvider>,
}

impl std::fmt::Debug for AgentOAuthState {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("AgentOAuthState")
            .field("config", &self.config)
            .field("auth_service", &"<AuthValidationService>")
            .field("db", &"<Database>")
            .field("jwt_provider", &self.jwt_provider.is_some())
            .field("user_provider", &self.user_provider.is_some())
            .finish()
    }
}

impl AgentOAuthState {
    pub fn new(
        db: Arc<Database>,
        config: AgentOAuthConfig,
        jwt_issuer: String,
        jwt_audiences: Vec<JwtAudience>,
    ) -> Self {
        Self {
            config,
            auth_service: Arc::new(AuthValidationService::new(jwt_issuer, jwt_audiences)),
            db,
            jwt_provider: None,
            user_provider: None,
        }
    }

    #[must_use]
    pub fn with_jwt_provider(mut self, provider: DynJwtValidationProvider) -> Self {
        self.jwt_provider = Some(provider);
        self
    }

    #[must_use]
    pub fn with_user_provider(mut self, provider: DynUserProvider) -> Self {
        self.user_provider = Some(provider);
        self
    }
}
