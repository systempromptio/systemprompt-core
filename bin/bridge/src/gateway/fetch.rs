//! Read-only gateway endpoints: pubkey, signed manifest, plugin files, whoami,
//! bridge profile, and the liveness probe.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Instant;

use systemprompt_models::api::cloud::{BridgeGovernanceDecisions, BridgeProfileUsage};

use crate::auth::types::BridgeProfile;
use crate::gateway::errors::GatewayError;
use crate::gateway::manifest::SignedManifestEnvelope;
use crate::gateway::types::{ReleaseManifest, WhoamiResponse};
use crate::gateway::{GatewayClient, record_span};

impl GatewayClient {
    #[tracing::instrument(
        level = "debug",
        skip(self),
        fields(endpoint = "pubkey", status, latency_ms)
    )]
    pub async fn fetch_pubkey(&self) -> Result<String, GatewayError> {
        #[derive(serde::Deserialize)]
        struct PubkeyResponse {
            #[serde(default)]
            pubkey: Option<String>,
        }
        let url = self.url("/v1/bridge/pubkey");
        let started = Instant::now();
        let resp = self
            .http()
            .get(&url)
            .send()
            .await
            .map_err(|e| GatewayError::PubkeyFetch(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "pubkey",
            });
        }
        let body: PubkeyResponse = resp
            .json()
            .await
            .map_err(|e| GatewayError::PubkeyDecode(Box::new(e)))?;
        body.pubkey.ok_or(GatewayError::PubkeyMissing)
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "manifest", status, latency_ms)
    )]
    pub async fn fetch_manifest(
        &self,
        bearer: &str,
    ) -> Result<SignedManifestEnvelope, GatewayError> {
        let url = self.url("/v1/bridge/manifest");
        let started = Instant::now();
        let resp = self
            .http()
            .get(&url)
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|e| GatewayError::ManifestFetch(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "manifest",
            });
        }
        let body = resp
            .text()
            .await
            .map_err(|e| GatewayError::ManifestDecode(Box::new(e)))?;
        serde_json::from_str::<SignedManifestEnvelope>(&body).map_err(|source| {
            GatewayError::ManifestEnvelopeShape {
                snippet: body.chars().take(120).collect(),
                source,
            }
        })
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(plugin_id, path, status, latency_ms)
    )]
    pub async fn fetch_plugin_file(
        &self,
        bearer: &str,
        plugin_id: &str,
        relative_path: &str,
    ) -> Result<Vec<u8>, GatewayError> {
        if relative_path.contains("..") || relative_path.starts_with('/') {
            return Err(GatewayError::UnsafePath(relative_path.to_owned()));
        }
        let url = self.url(&format!("/v1/bridge/plugins/{plugin_id}/{relative_path}"));
        let started = Instant::now();
        let resp = self
            .http()
            .get(&url)
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|e| GatewayError::PluginFetch {
                plugin_id: plugin_id.to_owned(),
                path: relative_path.to_owned(),
                source: Box::new(e),
            })?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "plugin",
            });
        }
        let bytes = resp.bytes().await.map_err(|e| GatewayError::PluginRead {
            plugin_id: plugin_id.to_owned(),
            path: relative_path.to_owned(),
            source: Box::new(e),
        })?;
        Ok(bytes.to_vec())
    }

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

    #[tracing::instrument(
        level = "debug",
        skip(self),
        fields(endpoint = "health", status, latency_ms)
    )]
    pub async fn health(&self) -> Result<(), GatewayError> {
        let url = self.url("/health");
        let started = Instant::now();
        let resp = self
            .http()
            .get(&url)
            .send()
            .await
            .map_err(|e| GatewayError::HealthCheck(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "health",
            });
        }
        Ok(())
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "bridge-latest", platform, status, latency_ms)
    )]
    pub async fn fetch_latest_release(
        &self,
        bearer: &str,
        platform: &str,
    ) -> Result<ReleaseManifest, GatewayError> {
        let url = self.url(&format!("/v1/bridge/latest?platform={platform}"));
        let started = Instant::now();
        let resp = self
            .http()
            .get(&url)
            .bearer_auth(bearer)
            .send()
            .await
            .map_err(|e| GatewayError::ReleaseFetch(Box::new(e)))?;
        record_span(&resp, started);
        if !resp.status().is_success() {
            return Err(GatewayError::HttpStatus {
                status: resp.status(),
                endpoint: "bridge-latest",
            });
        }
        resp.json::<ReleaseManifest>()
            .await
            .map_err(|e| GatewayError::ReleaseDecode(Box::new(e)))
    }
}
