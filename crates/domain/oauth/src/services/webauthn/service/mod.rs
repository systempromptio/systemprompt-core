mod authentication;
mod credentials;
mod registration;

pub use registration::FinishRegistrationParams;

use super::config::WebAuthnConfig;
use super::user_service::UserCreationService;
use crate::repository::OAuthRepository;
use anyhow::Result;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use systemprompt_traits::UserProvider;
use tokio::sync::Mutex;
use webauthn_rs::prelude::*;
use webauthn_rs::{Webauthn, WebauthnBuilder};

#[derive(Debug)]
pub(super) struct AuthenticationStateData {
    pub state: PasskeyAuthentication,
    pub user_id: String,
    pub oauth_state: Option<String>,
    pub timestamp: Instant,
}

pub struct WebAuthnService {
    pub(super) webauthn: Webauthn,
    pub(super) config: WebAuthnConfig,
    pub(super) oauth_repo: OAuthRepository,
    pub(super) user_creation_service: UserCreationService,
    pub(super) reg_states: Arc<Mutex<HashMap<String, (PasskeyRegistration, Instant)>>>,
    pub(super) auth_states: Arc<Mutex<HashMap<String, AuthenticationStateData>>>,
}

impl std::fmt::Debug for WebAuthnService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebAuthnService")
            .field("config", &self.config)
            .field("oauth_repo", &self.oauth_repo)
            .finish()
    }
}

impl WebAuthnService {
    pub fn new(oauth_repo: OAuthRepository, user_provider: Arc<dyn UserProvider>) -> Result<Self> {
        Self::with_config(WebAuthnConfig::new()?, oauth_repo, user_provider)
    }

    pub fn with_config(
        config: WebAuthnConfig,
        oauth_repo: OAuthRepository,
        user_provider: Arc<dyn UserProvider>,
    ) -> Result<Self> {
        let webauthn = WebauthnBuilder::new(&config.rp_id, &config.rp_origin)?
            .rp_name(&config.rp_name)
            .allow_any_port(config.allow_any_port)
            .allow_subdomains(config.allow_subdomains)
            .build()?;

        let user_creation_service = UserCreationService::new(user_provider);

        Ok(Self {
            webauthn,
            config,
            oauth_repo,
            user_creation_service,
            reg_states: Arc::new(Mutex::new(HashMap::new())),
            auth_states: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    pub async fn cleanup_expired_states(&self) -> Result<()> {
        let now = Instant::now();
        let expiry_duration = self.config.challenge_expiry;

        {
            let mut reg_states = self.reg_states.lock().await;
            reg_states.retain(|_challenge_id, (_state, timestamp)| {
                now.duration_since(*timestamp) < expiry_duration
            });
        }

        {
            let mut auth_states = self.auth_states.lock().await;
            auth_states
                .retain(|_challenge_id, data| now.duration_since(data.timestamp) < expiry_duration);
        }

        Ok(())
    }
}
