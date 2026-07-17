//! Linking new passkeys to existing user accounts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::WebAuthnService;
use crate::error::OauthResult as Result;
use crate::repository::TokenValidationResult;
use crate::services::webauthn::token::hash_token;
use std::collections::HashMap;
use std::sync::Arc;
use std::time::{Duration, Instant};
use systemprompt_identifiers::{TokenId, UserId};
use tokio::sync::Mutex;
use tracing::instrument;
use uuid::Uuid;
use webauthn_rs::prelude::*;

const CHALLENGE_EXPIRY_SECONDS: u64 = 300;

#[derive(Debug)]
pub struct LinkRegistrationState {
    pub reg_state: PasskeyRegistration,
    pub user_id: UserId,
    pub token_id: TokenId,
    pub timestamp: Instant,
}

pub type LinkStates = Arc<Mutex<HashMap<String, LinkRegistrationState>>>;

#[must_use]
pub fn create_link_states() -> LinkStates {
    Arc::new(Mutex::new(HashMap::new()))
}

#[derive(Debug, Clone)]
pub struct LinkUserInfo {
    pub id: UserId,
    pub email: String,
    pub name: String,
}

impl WebAuthnService {
    #[instrument(skip(self, setup_token, link_states))]
    pub async fn start_registration_with_token(
        &self,
        setup_token: &str,
        link_states: &LinkStates,
    ) -> Result<(CreationChallengeResponse, String, LinkUserInfo)> {
        let token_hash = hash_token(setup_token);
        let validation = self.oauth_repo.validate_setup_token(&token_hash).await?;

        let token_record = match validation {
            TokenValidationResult::Valid(record) => record,
            TokenValidationResult::Expired => {
                return Err(crate::error::OauthError::Internal(
                    "Setup token has expired".to_owned(),
                ));
            },
            TokenValidationResult::AlreadyUsed => {
                return Err(crate::error::OauthError::Internal(
                    "Setup token has already been used".to_owned(),
                ));
            },
            TokenValidationResult::NotFound => {
                return Err(crate::error::OauthError::Internal(
                    "Invalid setup token".to_owned(),
                ));
            },
        };

        let user = self
            .oauth_repo
            .get_authenticated_user(&token_record.user_id)
            .await?;

        let existing_creds = self.get_user_credentials(&token_record.user_id).await?;
        let exclude_credentials: Vec<CredentialID> =
            existing_creds.iter().map(|c| c.cred_id().clone()).collect();

        let user_unique_id = Uuid::parse_str(token_record.user_id.as_str()).map_err(|e| {
            crate::error::OauthError::Internal(format!(
                "user_id {:?} is not a valid UUID: {e}",
                token_record.user_id.as_str()
            ))
        })?;

        let (challenge, reg_state) = self.webauthn.start_passkey_registration(
            user_unique_id,
            &user.username,
            &user.username,
            if exclude_credentials.is_empty() {
                None
            } else {
                Some(exclude_credentials)
            },
        )?;

        let challenge_id = Uuid::new_v4().to_string();
        let state = LinkRegistrationState {
            reg_state,
            user_id: token_record.user_id.clone(),
            token_id: token_record.id.clone(),
            timestamp: Instant::now(),
        };

        {
            let mut states = link_states.lock().await;
            states.insert(challenge_id.clone(), state);
        }

        let user_info = LinkUserInfo {
            id: token_record.user_id.clone(),
            email: user.email,
            name: user.username,
        };

        tracing::info!(
            user_id = %user_info.id,
            challenge_id = %challenge_id,
            "Link registration ceremony initiated"
        );

        Ok((challenge, challenge_id, user_info))
    }

    #[instrument(skip(self, setup_token, credential, link_states))]
    pub async fn finish_registration_with_token(
        &self,
        challenge_id: &str,
        setup_token: &str,
        credential: &RegisterPublicKeyCredential,
        link_states: &LinkStates,
    ) -> Result<UserId> {
        let token_hash = hash_token(setup_token);
        let validation = self.oauth_repo.validate_setup_token(&token_hash).await?;

        let TokenValidationResult::Valid(token_record) = validation else {
            return Err(crate::error::OauthError::Internal(
                "Invalid or expired setup token".to_owned(),
            ));
        };

        let state = {
            let mut states = link_states.lock().await;
            states.remove(challenge_id).ok_or_else(|| {
                crate::error::OauthError::Internal(
                    "Registration session not found or expired".to_owned(),
                )
            })?
        };

        if state.token_id != token_record.id {
            return Err(crate::error::OauthError::Internal(
                "Token mismatch".to_owned(),
            ));
        }

        if state.timestamp.elapsed() > Duration::from_secs(CHALLENGE_EXPIRY_SECONDS) {
            return Err(crate::error::OauthError::Internal(
                "Registration session expired".to_owned(),
            ));
        }

        let passkey = self
            .webauthn
            .finish_passkey_registration(credential, &state.reg_state)?;

        self.store_credential(&state.user_id, &passkey, "Linked Passkey")
            .await?;

        self.oauth_repo
            .consume_setup_token(&token_record.id)
            .await?;

        tracing::info!(
            user_id = %state.user_id,
            "WebAuthn credential linked to existing user"
        );

        Ok(state.user_id)
    }
}
