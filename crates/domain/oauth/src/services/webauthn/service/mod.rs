//! Inner `WebAuthn` service: registration, authentication, link, credentials.

mod authentication;
mod credentials;
mod link;
mod registration;

pub use link::{LinkStates, LinkUserInfo, create_link_states};
pub use registration::FinishRegistrationParams;

use std::time::Duration;

use super::config::WebAuthnConfig;
use super::user_service::UserCreationService;
use crate::error::OauthResult as Result;
use crate::repository::OAuthRepository;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::Instant;
use systemprompt_identifiers::UserId;
use systemprompt_traits::UserProvider;
use tokio::sync::Mutex;
use webauthn_rs::prelude::*;
use webauthn_rs::{Webauthn, WebauthnBuilder};

fn cap_by_age<K: Clone + std::hash::Hash + Eq, V, F: Fn(&V) -> Instant>(
    map: &mut HashMap<K, V>,
    max: usize,
    key_ts: F,
) {
    if map.len() <= max {
        return;
    }
    let mut keyed: Vec<(K, Instant)> = map.iter().map(|(k, v)| (k.clone(), key_ts(v))).collect();
    keyed.sort_by_key(|(_, ts)| *ts);
    let to_drop = map.len() - max;
    for (k, _) in keyed.into_iter().take(to_drop) {
        map.remove(&k);
    }
}

#[derive(Debug)]
pub(super) struct AuthenticationStateData {
    pub state: PasskeyAuthentication,
    pub user_id: UserId,
    pub oauth_state: Option<String>,
    pub timestamp: Instant,
}

#[derive(Debug, Clone)]
pub struct VerifiedAuthentication {
    pub user_id: UserId,
    pub timestamp: Instant,
}

pub struct WebAuthnService {
    pub(super) webauthn: Webauthn,
    pub(super) config: WebAuthnConfig,
    pub(super) oauth_repo: OAuthRepository,
    pub(super) user_creation_service: UserCreationService,
    pub(super) reg_states: Arc<Mutex<HashMap<String, (PasskeyRegistration, Instant)>>>,
    pub(super) auth_states: Arc<Mutex<HashMap<String, AuthenticationStateData>>>,
    pub(super) verified_auths: Arc<Mutex<HashMap<String, VerifiedAuthentication>>>,
}

impl std::fmt::Debug for WebAuthnService {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("WebAuthnService")
            .field("config", &self.config)
            .field("oauth_repo", &self.oauth_repo)
            .finish_non_exhaustive()
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
            verified_auths: Arc::new(Mutex::new(HashMap::new())),
        })
    }

    /// Hard cap on pending challenges per kind. Acts as a defence against
    /// unbounded growth between [`Self::cleanup_expired_states`] sweeps if an
    /// attacker initiates many half-finished `WebAuthn` ceremonies.
    pub const MAX_PENDING_CHALLENGES: usize = 10_000;

    pub async fn cleanup_expired_states(&self) -> Result<()> {
        let now = Instant::now();
        let expiry_duration = self.config.challenge_expiry;

        {
            let mut reg_states = self.reg_states.lock().await;
            reg_states.retain(|_challenge_id, (_state, timestamp)| {
                now.duration_since(*timestamp) < expiry_duration
            });
            cap_by_age(&mut *reg_states, Self::MAX_PENDING_CHALLENGES, |v| v.1);
        }

        {
            let mut auth_states = self.auth_states.lock().await;
            auth_states
                .retain(|_challenge_id, data| now.duration_since(data.timestamp) < expiry_duration);
            cap_by_age(&mut *auth_states, Self::MAX_PENDING_CHALLENGES, |v| {
                v.timestamp
            });
        }

        {
            let mut verified = self.verified_auths.lock().await;
            verified.retain(|_token, data| now.duration_since(data.timestamp) < expiry_duration);
            cap_by_age(&mut *verified, Self::MAX_PENDING_CHALLENGES, |v| {
                v.timestamp
            });
        }

        Ok(())
    }

    pub async fn store_verified_authentication(&self, token: String, user_id: UserId) {
        let mut verified = self.verified_auths.lock().await;
        verified.insert(
            token,
            VerifiedAuthentication {
                user_id,
                timestamp: Instant::now(),
            },
        );
    }

    pub async fn consume_verified_authentication(&self, token: &str) -> Result<UserId> {
        let data = {
            let mut verified = self.verified_auths.lock().await;
            verified.remove(token).ok_or_else(|| {
                crate::error::OauthError::Internal(
                    "No verified authentication found for token".to_string(),
                )
            })?
        };

        if data.timestamp.elapsed() > Duration::from_secs(120) {
            return Err(crate::error::OauthError::Internal(
                "Verified authentication token expired".to_string(),
            ));
        }

        Ok(data.user_id)
    }
}
