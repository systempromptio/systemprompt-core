use crate::services::shared::{AgentSessionUser, Result};
use std::sync::Arc;
use systemprompt_database::Database;
use systemprompt_models::auth::JwtAudience;
pub use systemprompt_models::AgentOAuthConfig;
use systemprompt_security::{AuthMode, AuthValidationService};
use systemprompt_traits::{DynJwtValidationProvider, DynUserProvider};

pub type AgentAuthenticatedUser = AgentSessionUser;

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
    pub async fn new(
        db: Arc<Database>,
        config: AgentOAuthConfig,
        jwt_secret: String,
        jwt_issuer: String,
        jwt_audiences: Vec<JwtAudience>,
    ) -> Result<Self> {
        Ok(Self {
            config,
            auth_service: Arc::new(AuthValidationService::new(
                jwt_secret,
                jwt_issuer,
                jwt_audiences,
            )),
            db,
            jwt_provider: None,
            user_provider: None,
        })
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

    pub const fn auth_mode(&self) -> AuthMode {
        if self.config.required {
            AuthMode::Required
        } else {
            AuthMode::Optional
        }
    }
}
