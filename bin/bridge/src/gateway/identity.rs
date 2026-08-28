//! Gateway endpoints describing *who this bridge is* and what its plan allows:
//! whoami, the bridge profile, token usage, governance decisions, and the
//! per-host model filter.
//!
//! Split from `fetch.rs`, which keeps the artefact endpoints — pubkey, signed
//! manifest, plugin files and releases.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use systemprompt_models::api::cloud::{BridgeGovernanceDecisions, BridgeProfileUsage};

use crate::auth::types::BridgeProfile;
use crate::gateway::errors::GatewayError;
use crate::gateway::types::WhoamiResponse;
use crate::gateway::{GatewayClient, record_span};

impl GatewayClient {
    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "whoami", status, latency_ms)
    )]
    pub async fn fetch_whoami(&self, bearer: &str) -> Result<WhoamiResponse, GatewayError> {
        let url = self.url("/v1/bridge/whoami");
        let started = Instant::now();
        let resp = self
            .http()
            .get(&url)
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|e| GatewayError::WhoamiFetch(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "whoami",
            });
        }
        resp.json::<WhoamiResponse>()
            .await
            .map_err(|e| GatewayError::WhoamiDecode(Box::new(e)))
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "host-model-filter", status, latency_ms)
    )]
    pub async fn set_host_model_filter(
        &self,
        bearer: &str,
        host_id: &str,
        protocols: Option<&[String]>,
    ) -> Result<(), GatewayError> {
        let url = self.url("/v1/bridge/profile/host-model-filter");
        let body = serde_json::json!({
            "host_id": host_id,
            "model_protocols": protocols,
        });
        let started = Instant::now();
        let resp = self
            .http()
            .post(&url)
            .bearer_auth(bearer)
            .json(&body)
            .send()
            .await
            .map_err(|e| GatewayError::PostRequest(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "host-model-filter",
            });
        }
        Ok(())
    }

    #[tracing::instrument(
        level = "debug",
        skip(self),
        fields(endpoint = "profile", status, latency_ms)
    )]
    pub async fn fetch_bridge_profile(&self) -> Result<BridgeProfile, GatewayError> {
        let url = self.url("/v1/bridge/profile");
        let started = Instant::now();
        let resp = self
            .http()
            .get(&url)
            .send()
            .await
            .map_err(|e| GatewayError::ProfileFetch(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "profile",
            });
        }
        resp.json::<BridgeProfile>()
            .await
            .map_err(|e| GatewayError::ProfileDecode(Box::new(e)))
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "profile_usage", status, latency_ms)
    )]
    pub async fn fetch_profile_usage(
        &self,
        bearer: &str,
    ) -> Result<BridgeProfileUsage, GatewayError> {
        let url = self.url("/v1/bridge/profile/usage");
        let started = Instant::now();
        let resp = self
            .http()
            .get(&url)
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|e| GatewayError::ProfileUsageFetch(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "profile_usage",
            });
        }
        resp.json::<BridgeProfileUsage>()
            .await
            .map_err(|e| GatewayError::ProfileUsageDecode(Box::new(e)))
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "decisions", status, latency_ms)
    )]
    pub async fn fetch_decisions(
        &self,
        bearer: &str,
        since_unix: u64,
    ) -> Result<BridgeGovernanceDecisions, GatewayError> {
        let url = self.url(&format!("/v1/bridge/decisions?since={since_unix}"));
        let started = Instant::now();
        let resp = self
            .http()
            .get(&url)
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|e| GatewayError::DecisionsFetch(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "decisions",
            });
        }
        resp.json::<BridgeGovernanceDecisions>()
            .await
            .map_err(|e| GatewayError::DecisionsDecode(Box::new(e)))
    }
}
