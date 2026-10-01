//! Linking new passkeys to existing user accounts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::WebAuthnService;
use crate::error::{OauthError, OauthResult as Result};
use crate::repository::{ReserveLinkChallengeParams, TokenValidationResult, WebAuthnChallengeKind};
use crate::services::load_authenticated_user;
use crate::services::webauthn::token::hash_token;
use serde::{Deserialize, Serialize};
use std::time::Duration;
use systemprompt_identifiers::{ChallengeId, TokenId, UserId};
use tracing::instrument;
use uuid::Uuid;
use webauthn_rs::prelude::*;

const LINK_CHALLENGE_TTL: Duration = Duration::from_secs(300);
const LINK_CHALLENGE_MIN_REMAINING: Duration = Duration::from_secs(60);
// Why: webauthn-rs 0.5 takes the passkey user handle as a `Uuid`, while user
// ids are opaque text, so the handle is a name-based UUID of the user id.
const PASSKEY_USER_HANDLE_NAMESPACE: Uuid =
    Uuid::from_u128(0x5e0c_6b7a_2f1d_4c3e_9a8b_7d6c_5b4a_3f2e);

fn passkey_user_handle(user_id: &UserId) -> Uuid {
    Uuid::new_v5(&PASSKEY_USER_HANDLE_NAMESPACE, user_id.as_str().as_bytes())
}

#[derive(Debug, Serialize, Deserialize)]
struct LinkRegistrationState {
    reg_state: PasskeyRegistration,
    token_id: TokenId,
    challenge: CreationChallengeResponse,
}

#[derive(Debug, Clone)]
pub struct LinkUserInfo {
    pub id: UserId,
    pub email: String,
    pub name: String,
}

impl WebAuthnService {
    #[instrument(skip(self, setup_token))]
    pub async fn start_registration_with_token(
        &self,
        setup_token: &str,
    ) -> Result<(CreationChallengeResponse, ChallengeId, LinkUserInfo)> {
        let token_hash = hash_token(setup_token);
        let validation = self.oauth_repo.validate_setup_token(&token_hash).await?;

        let token_record = match validation {
            TokenValidationResult::Valid(record) => record,
            TokenValidationResult::Expired => {
                return Err(OauthError::SetupTokenRejected("Setup token has expired"));
            },
            TokenValidationResult::AlreadyUsed => {
                return Err(OauthError::SetupTokenRejected(
                    "Setup token has already been used",
                ));
            },
            TokenValidationResult::NotFound => {
                return Err(OauthError::SetupTokenRejected("Invalid setup token"));
            },
        };

        let user =
            load_authenticated_user(self.user_provider.as_ref(), &token_record.user_id).await?;

        let existing_creds = self.get_user_credentials(&token_record.user_id).await?;
        let exclude_credentials: Vec<CredentialID> =
            existing_creds.iter().map(|c| c.cred_id().clone()).collect();

        let user_unique_id = passkey_user_handle(&token_record.user_id);

        let reservation = self
            .oauth_repo
            .reserve_link_challenge(
                ReserveLinkChallengeParams {
                    user_id: &token_record.user_id,
                    token_id: &token_record.id,
                    ttl: LINK_CHALLENGE_TTL,
                    min_remaining: LINK_CHALLENGE_MIN_REMAINING,
                },
                |_challenge_id| {
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
                    Ok(serde_json::to_value(LinkRegistrationState {
                        reg_state,
                        token_id: token_record.id.clone(),
                        challenge,
                    })?)
                },
            )
            .await?;
        let state: LinkRegistrationState = serde_json::from_value(reservation.state)?;
        let challenge_id = reservation.challenge_id;

        let user_info = LinkUserInfo {
            id: token_record.user_id.clone(),
            email: user.email,
            name: user.username,
        };

        tracing::info!(
            user_id = %user_info.id,
            challenge_id = %challenge_id,
            reused = reservation.reused,
            "Link registration ceremony initiated"
        );

        Ok((state.challenge, challenge_id, user_info))
    }

    #[instrument(skip(self, setup_token, credential))]
    pub async fn finish_registration_with_token(
        &self,
        challenge_id: &ChallengeId,
        setup_token: &str,
        credential: &RegisterPublicKeyCredential,
    ) -> Result<UserId> {
        let token_hash = hash_token(setup_token);
        let validation = self.oauth_repo.validate_setup_token(&token_hash).await?;

        let TokenValidationResult::Valid(token_record) = validation else {
            return Err(OauthError::SetupTokenRejected(
                "Invalid or expired setup token",
            ));
        };

        let consumed = self
            .oauth_repo
            .consume_webauthn_challenge(challenge_id.as_str(), WebAuthnChallengeKind::Link)
            .await?
            .ok_or(OauthError::RegistrationStateExpired)?;
        let user_id = consumed
            .user_id
            .ok_or(OauthError::Internal("Registration session has no user id"))?;
        let state: LinkRegistrationState = serde_json::from_value(consumed.state)?;

        if state.token_id != token_record.id {
            return Err(OauthError::SetupTokenRejected("Token mismatch"));
        }

        let passkey = self
            .webauthn
            .finish_passkey_registration(credential, &state.reg_state)?;

        self.store_credential(&user_id, &passkey, "Linked Passkey")
            .await?;

        self.oauth_repo
            .consume_setup_token(&token_record.id)
            .await?;

        tracing::info!(
            user_id = %user_id,
            "WebAuthn credential linked to existing user"
        );

        Ok(user_id)
    }
}
