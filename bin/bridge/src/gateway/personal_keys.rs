//! Gateway endpoints for the caller's personal credentials.
//!
//! `/v1/me/provider-credentials` is keyed by credential id: list which
//! personal credentials hold a key, store one (the gateway verifies it
//! upstream first) and remove it. The key travels only in the PUT body;
//! nothing here logs it.
//!
//! [`personal_key_rows`] is what the GUI lists: one row per credential, with
//! every provider endpoint that one key serves, so a credential serving
//! several endpoints (`OpenCode` Go) is asked for once.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use serde::Serialize;
use systemprompt_identifiers::CredentialId;
use systemprompt_models::bridge::profile::{
    ProviderCredentialStatus, ProviderHealth, SetProviderCredentialRequest,
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
        credential: &CredentialId,
        body: &SetProviderCredentialRequest,
    ) -> Result<ProviderCredentialStatus, GatewayError> {
        let started = Instant::now();
        let resp = self
            .http()
            .put(self.url(&format!("{PATH}/{}", credential.as_str())))
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
        credential: &CredentialId,
    ) -> Result<(), GatewayError> {
        let started = Instant::now();
        let resp = self
            .http()
            .delete(self.url(&format!("{PATH}/{}", credential.as_str())))
            .bearer_auth(bearer.expose())
            .send()
            .await
            .map_err(|e| GatewayError::PostRequest(Box::new(e)))?;
        record_span(&resp, started);
        ensure_success(resp, "provider-credentials").await?;
        Ok(())
    }
}

/// One personal credential as the GUI lists it.
#[derive(Debug, Clone, Serialize)]
pub struct PersonalKeyRow {
    #[serde(flatten)]
    pub status: ProviderCredentialStatus,
    pub endpoints: Vec<String>,
}

#[must_use]
pub fn personal_key_rows(
    statuses: Vec<ProviderCredentialStatus>,
    providers: &[ProviderHealth],
) -> Vec<PersonalKeyRow> {
    let mut rows: Vec<PersonalKeyRow> = Vec::with_capacity(statuses.len());
    for status in statuses {
        if rows
            .iter()
            .any(|r| r.status.credential == status.credential)
        {
            continue;
        }
        let mut endpoints: Vec<String> = Vec::new();
        for provider in providers
            .iter()
            .filter(|p| p.personal_key && p.credential == status.credential)
        {
            if !endpoints.contains(&provider.name) {
                endpoints.push(provider.name.clone());
            }
        }
        rows.push(PersonalKeyRow { status, endpoints });
    }
    rows
}
