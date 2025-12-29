use super::WebAuthnService;
use anyhow::Result;
use base64::engine::{general_purpose, Engine};
use std::time::Instant;
use tracing::instrument;
use uuid::Uuid;
use webauthn_rs::prelude::*;

#[derive(Debug)]
pub struct FinishRegistrationParams<'a> {
    pub challenge_id: &'a str,
    pub username: &'a str,
    pub email: &'a str,
    pub full_name: Option<&'a str>,
    pub reg_response: &'a RegisterPublicKeyCredential,
}

#[derive(Debug)]
pub struct FinishRegistrationParamsBuilder<'a> {
    challenge_id: &'a str,
    username: &'a str,
    email: &'a str,
    full_name: Option<&'a str>,
    reg_response: &'a RegisterPublicKeyCredential,
}

impl<'a> FinishRegistrationParamsBuilder<'a> {
    pub const fn new(
        challenge_id: &'a str,
        username: &'a str,
        email: &'a str,
        reg_response: &'a RegisterPublicKeyCredential,
    ) -> Self {
        Self {
            challenge_id,
            username,
            email,
            full_name: None,
            reg_response,
        }
    }

    pub const fn with_full_name(mut self, full_name: &'a str) -> Self {
        self.full_name = Some(full_name);
        self
    }

    pub const fn build(self) -> FinishRegistrationParams<'a> {
        FinishRegistrationParams {
            challenge_id: self.challenge_id,
            username: self.username,
            email: self.email,
            full_name: self.full_name,
            reg_response: self.reg_response,
        }
    }
}

impl<'a> FinishRegistrationParams<'a> {
    pub const fn builder(
        challenge_id: &'a str,
        username: &'a str,
        email: &'a str,
        reg_response: &'a RegisterPublicKeyCredential,
    ) -> FinishRegistrationParamsBuilder<'a> {
        FinishRegistrationParamsBuilder::new(challenge_id, username, email, reg_response)
    }
}

impl WebAuthnService {
    #[instrument(skip(self), fields(username = %username, email = %email))]
    pub async fn start_registration(
        &self,
        username: &str,
        email: &str,
        full_name: Option<&str>,
    ) -> Result<(CreationChallengeResponse, String)> {
        let user_unique_id = Uuid::new_v4();
        let display_name = full_name.filter(|n| !n.is_empty()).unwrap_or(username);

        let exclude_credentials = self.get_user_credentials_by_email(email).await?;

        let exclude_cred_ids: Vec<_> = exclude_credentials
            .iter()
            .map(|pk| pk.cred_id().clone())
            .collect();

        let exclude_cred_ids_len = exclude_cred_ids.len();

        let (ccr, reg_state) = self.webauthn.start_passkey_registration(
            user_unique_id,
            username,
            display_name,
            if exclude_cred_ids.is_empty() {
                None
            } else {
                Some(exclude_cred_ids)
            },
        )?;

        let challenge_id = Uuid::new_v4().to_string();

        {
            let mut states = self.reg_states.lock().await;
            states.insert(challenge_id.clone(), (reg_state, Instant::now()));
        }

        tracing::info!(
            username = %username,
            user_email = %email,
            challenge_id = %challenge_id,
            user_unique_id = %user_unique_id,
            display_name = %display_name,
            full_name = ?full_name,
            excluded_credentials_count = exclude_cred_ids_len,
            "Registration ceremony initiated"
        );

        Ok((ccr, challenge_id))
    }

    #[instrument(skip(self, params), fields(challenge_id = %params.challenge_id, username = %params.username))]
    pub async fn finish_registration(
        &self,
        params: FinishRegistrationParams<'_>,
    ) -> Result<String> {
        let reg_state = self
            .retrieve_and_remove_registration_state(params.challenge_id)
            .await?;

        match self
            .webauthn
            .finish_passkey_registration(params.reg_response, &reg_state)
        {
            Ok(sk) => {
                let user_id = self
                    .user_creation_service
                    .create_user_with_webauthn_registration(
                        params.username,
                        params.email,
                        params.full_name,
                    )
                    .await?;

                let credential_id = sk.cred_id().clone();
                let display_name = params
                    .full_name
                    .filter(|n| !n.is_empty())
                    .unwrap_or(params.username);
                self.complete_registration(&user_id, &sk, display_name)
                    .await?;

                tracing::info!(
                    username = %params.username,
                    user_email = %params.email,
                    user_id = %user_id,
                    challenge_id = %params.challenge_id,
                    credential_id = %general_purpose::STANDARD.encode(&credential_id),
                    display_name = %display_name,
                    full_name = ?params.full_name,
                    counter = 0,
                    "WebAuthn registration completed"
                );

                Ok(user_id)
            },
            Err(e) => {
                tracing::info!(
                    username = %params.username,
                    user_email = %params.email,
                    challenge_id = %params.challenge_id,
                    failure_reason = %e,
                    full_name = ?params.full_name,
                    "WebAuthn registration failed"
                );
                Err(e.into())
            },
        }
    }

    async fn retrieve_and_remove_registration_state(
        &self,
        challenge_id: &str,
    ) -> Result<PasskeyRegistration> {
        let mut states = self.reg_states.lock().await;
        states
            .remove(challenge_id)
            .map(|(state, _timestamp)| state)
            .ok_or_else(|| anyhow::anyhow!("Registration state not found or expired"))
    }

    async fn complete_registration(
        &self,
        user_id: &str,
        sk: &Passkey,
        display_name: &str,
    ) -> Result<()> {
        self.store_credential(user_id, sk, display_name).await
    }
}
