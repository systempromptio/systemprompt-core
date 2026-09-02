//! Inner `WebAuthn` service: registration, authentication, link, credentials.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod authentication;
mod credentials;
mod link;
mod registration;

pub use credentials::normalize_transport_casing;
pub use link::LinkUserInfo;
pub use registration::FinishRegistrationParams;

use std::time::Duration;

use super::config::WebAuthnConfig;
use super::user_service::UserCreationService;
use crate::error::{OauthError, OauthResult as Result};
use crate::repository::{OAuthRepository, StoreChallengeParams, WebAuthnChallengeKind};
use sha2::{Digest, Sha256};
use std::sync::Arc;
use systemprompt_identifiers::UserId;
use systemprompt_traits::UserProvider;
use webauthn_rs::{Webauthn, WebauthnBuilder};

const VERIFIED_AUTHENTICATION_TTL: Duration = Duration::from_secs(120);

fn verified_token_key(token: &str) -> String {
    let mut hasher = Sha256::new();
    hasher.update(token.as_bytes());
    hex::encode(hasher.finalize())
}

pub struct WebAuthnService {
    pub(super) webauthn: Webauthn,
    pub(super) config: WebAuthnConfig,
    pub(super) oauth_repo: OAuthRepository,
    pub(super) user_creation_service: UserCreationService,
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
        })
    }

    pub async fn cleanup_expired_states(&self) -> Result<()> {
        let removed = self
            .oauth_repo
            .cleanup_expired_webauthn_challenges()
            .await?;
        if removed > 0 {
            tracing::debug!(removed, "Expired WebAuthn challenges purged");
        }
        Ok(())
    }

    pub async fn store_verified_authentication(
        &self,
        token: String,
        user_id: UserId,
    ) -> Result<()> {
        let key = verified_token_key(&token);
        self.oauth_repo
            .store_webauthn_challenge(StoreChallengeParams {
                challenge: &key,
                kind: WebAuthnChallengeKind::Verified,
                user_id: Some(&user_id),
                state: &serde_json::Value::Null,
                oauth_state: None,
                ttl: VERIFIED_AUTHENTICATION_TTL,
            })
            .await
    }

    pub async fn consume_verified_authentication(&self, token: &str) -> Result<UserId> {
        let key = verified_token_key(token);
        let consumed = self
            .oauth_repo
            .consume_webauthn_challenge(&key, WebAuthnChallengeKind::Verified)
            .await?
            .ok_or_else(|| {
                OauthError::Internal("No verified authentication found for token".to_owned())
            })?;

        consumed.user_id.ok_or_else(|| {
            OauthError::Internal("Verified authentication has no user id".to_owned())
        })
    }
}
