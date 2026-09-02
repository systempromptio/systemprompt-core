//! `WebAuthn` passkey authentication flow.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::WebAuthnService;
use crate::error::{OauthError, OauthResult as Result};
use crate::repository::{StoreChallengeParams, WebAuthnChallengeKind};
use base64::engine::{Engine, general_purpose};
use systemprompt_identifiers::UserId;
use tracing::instrument;
use uuid::Uuid;
use webauthn_rs::prelude::*;

impl WebAuthnService {
    #[instrument(skip(self), fields(email = %email))]
    pub async fn start_authentication(
        &self,
        email: &str,
        oauth_state: Option<String>,
    ) -> Result<(RequestChallengeResponse, String)> {
        let user = self
            .oauth_repo
            .find_user_by_email(email)
            .await?
            .ok_or_else(|| OauthError::UserNotFound(email.to_owned()))?;

        let user_credentials = self.get_user_credentials(&user.id).await?;

        if user_credentials.is_empty() {
            return Err(OauthError::Internal(
                "No credentials found for user".to_owned(),
            ));
        }

        let (rcr, auth_state) = self
            .webauthn
            .start_passkey_authentication(&user_credentials)?;

        let challenge_id = Uuid::new_v4().to_string();

        let state = serde_json::to_value(&auth_state)?;
        self.oauth_repo
            .store_webauthn_challenge(StoreChallengeParams {
                challenge: &challenge_id,
                kind: WebAuthnChallengeKind::Authentication,
                user_id: Some(&user.id),
                state: &state,
                oauth_state: oauth_state.as_deref(),
                ttl: self.config.challenge_expiry,
            })
            .await?;

        tracing::info!(
            user_email = %email,
            user_id = %user.id,
            challenge_id = %challenge_id,
            available_credentials = user_credentials.len(),
            oauth_state_present = oauth_state.is_some(),
            "Authentication ceremony initiated"
        );

        Ok((rcr, challenge_id))
    }

    #[instrument(skip(self, auth_response), fields(challenge_id = %challenge_id))]
    pub async fn finish_authentication(
        &self,
        challenge_id: &str,
        auth_response: &PublicKeyCredential,
    ) -> Result<(UserId, Option<String>)> {
        let (auth_state, user_id, oauth_state) = self
            .retrieve_and_remove_authentication_state(challenge_id)
            .await?;

        match self
            .webauthn
            .finish_passkey_authentication(auth_response, &auth_state)
        {
            Ok(auth_result) => {
                self.complete_authentication(&auth_result, challenge_id)
                    .await?;

                tracing::info!(
                    user_id = %user_id,
                    challenge_id = %challenge_id,
                    credential_id = %general_purpose::STANDARD.encode(auth_result.cred_id().as_ref()),
                    counter = auth_result.counter(),
                    oauth_state_present = oauth_state.is_some(),
                    "WebAuthn authentication successful"
                );

                Ok((user_id, oauth_state))
            },
            Err(e) => {
                tracing::info!(
                    user_id = %user_id,
                    challenge_id = %challenge_id,
                    failure_reason = %e,
                    attempt_count = 1,
                    "WebAuthn authentication failed"
                );

                Err(e.into())
            },
        }
    }

    async fn retrieve_and_remove_authentication_state(
        &self,
        challenge_id: &str,
    ) -> Result<(PasskeyAuthentication, UserId, Option<String>)> {
        let consumed = self
            .oauth_repo
            .consume_webauthn_challenge(challenge_id, WebAuthnChallengeKind::Authentication)
            .await?
            .ok_or_else(|| {
                OauthError::Internal("Authentication state not found or expired".to_owned())
            })?;

        let user_id = consumed.user_id.ok_or_else(|| {
            OauthError::Internal("Authentication state has no user id".to_owned())
        })?;
        let state: PasskeyAuthentication = serde_json::from_value(consumed.state)?;

        Ok((state, user_id, consumed.oauth_state))
    }

    async fn complete_authentication(
        &self,
        auth_result: &AuthenticationResult,
        _challenge_id: &str,
    ) -> Result<()> {
        let cred_id = auth_result.cred_id();
        self.update_credential_counter(cred_id.as_ref(), auth_result.counter())
            .await?;
        Ok(())
    }
}
