use super::{AuthenticationStateData, WebAuthnService};
use anyhow::Result;
use base64::engine::{Engine, general_purpose};
use std::time::Instant;
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
            .ok_or_else(|| anyhow::anyhow!("User not found"))?;

        let user_credentials = self.get_user_credentials(&user.id).await?;

        if user_credentials.is_empty() {
            return Err(anyhow::anyhow!("No credentials found for user"));
        }

        let (rcr, auth_state) = self
            .webauthn
            .start_passkey_authentication(&user_credentials)?;

        let challenge_id = Uuid::new_v4().to_string();

        {
            let mut states = self.auth_states.lock().await;
            states.insert(
                challenge_id.clone(),
                AuthenticationStateData {
                    state: auth_state,
                    user_id: user.id.clone(),
                    oauth_state: oauth_state.clone(),
                    timestamp: Instant::now(),
                },
            );
        }

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
        let data = {
            let mut states = self.auth_states.lock().await;
            states
                .remove(challenge_id)
                .ok_or_else(|| anyhow::anyhow!("Authentication state not found or expired"))?
        };

        if data.timestamp.elapsed() > std::time::Duration::from_secs(120) {
            return Err(anyhow::anyhow!("Authentication challenge expired"));
        }

        Ok((data.state, data.user_id, data.oauth_state))
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
