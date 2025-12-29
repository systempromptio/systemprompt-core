use crate::services::shared::{AgentSessionUser, Result};
use std::sync::Arc;
use systemprompt_core_database::Database;
use systemprompt_core_security::{AuthMode, AuthValidationService};
use systemprompt_models::auth::JwtAudience;
pub use systemprompt_models::AgentOAuthConfig;

pub type AgentAuthenticatedUser = AgentSessionUser;

#[derive(Debug, Clone)]
pub struct AgentOAuthState {
    pub config: AgentOAuthConfig,
    pub auth_service: Arc<AuthValidationService>,
    pub db: Arc<Database>,
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
        })
    }

    pub const fn auth_mode(&self) -> AuthMode {
        if self.config.required {
            AuthMode::Required
        } else {
            AuthMode::Optional
        }
    }
}
