//! Gateway endpoints for the caller's personal provider keys
//! (`/v1/me/provider-credentials`): list which personal-subscription providers
//! hold a key, store one (the gateway verifies it upstream first) and remove
//! it. The key travels only in the PUT body; nothing here logs it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use systemprompt_models::bridge::profile::{
    ProviderCredentialStatus, SetProviderCredentialRequest,
};

use crate::gateway::errors::GatewayError;
use crate::gateway::{GatewayClient, ensure_success, record_span};
use crate::ids::BearerToken;

const PATH: &str = systemprompt_models::ApiPaths::BRIDGE_PERSONAL_KEYS;

impl GatewayClient {
    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "provider-credentials", status, latency_ms)
    )]
    pub async fn list_personal_keys(
        &self,
        bearer: &BearerToken,
    ) -> Result<Vec<ProviderCredentialStatus>, GatewayError> {
        let started = Instant::now();
        let resp = self
            .http()
            .get(self.url(PATH))
            .bearer_auth(bearer.expose())
            .send()
            .await
            .map_err(|e| GatewayError::PostRequest(Box::new(e)))?;
        record_span(&resp, started);
        let resp = ensure_success(resp, "provider-credentials").await?;
        resp.json()
            .await
            .map_err(|e| GatewayError::AuthDecode(Box::new(e)))
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer, body),
        fields(endpoint = "provider-credentials", status, latency_ms)
    )]
    pub async fn set_personal_key(
        &self,
        bearer: &BearerToken,
        provider: &str,
        body: &SetProviderCredentialRequest,
    ) -> Result<ProviderCredentialStatus, GatewayError> {
        let started = Instant::now();
        let resp = self
            .http()
            .put(self.url(&format!("{PATH}/{provider}")))
            .bearer_auth(bearer.expose())
            .json(body)
            .send()
            .await
            .map_err(|e| GatewayError::PostRequest(Box::new(e)))?;
        record_span(&resp, started);
        let resp = ensure_success(resp, "provider-credentials").await?;
        resp.json()
            .await
            .map_err(|e| GatewayError::AuthDecode(Box::new(e)))
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "provider-credentials", status, latency_ms)
    )]
    pub async fn delete_personal_key(
        &self,
        bearer: &BearerToken,
        provider: &str,
    ) -> Result<(), GatewayError> {
        let started = Instant::now();
        let resp = self
            .http()
            .delete(self.url(&format!("{PATH}/{provider}")))
            .bearer_auth(bearer.expose())
            .send()
            .await
            .map_err(|e| GatewayError::PostRequest(Box::new(e)))?;
        record_span(&resp, started);
        ensure_success(resp, "provider-credentials").await?;
        Ok(())
    }
}
