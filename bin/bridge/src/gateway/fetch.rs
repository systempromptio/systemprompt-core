//! Read-only gateway endpoints.
//!
//! GET-style operations the bridge uses to discover gateway state: signing
//! pubkey, signed manifest, plugin file payloads, the caller's identity
//! (`whoami`), the bridge profile snapshot, and the liveness probe. All
//! results decode into types from `types` or `manifest`. Auth-mutating
//! exchanges live in `auth`.

use std::time::Instant;

use systemprompt_models::api::cloud::BridgeProfileUsage;

use crate::auth::types::BridgeProfile;
use crate::gateway::errors::GatewayError;
use crate::gateway::manifest::SignedManifest;
use crate::gateway::types::WhoamiResponse;
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
    pub async fn fetch_manifest(&self, bearer: &str) -> Result<SignedManifest, GatewayError> {
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
        resp.json::<SignedManifest>()
            .await
            .map_err(|e| GatewayError::ManifestDecode(Box::new(e)))
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
            return Err(GatewayError::UnsafePath(relative_path.to_string()));
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
                plugin_id: plugin_id.to_string(),
                path: relative_path.to_string(),
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
            plugin_id: plugin_id.to_string(),
            path: relative_path.to_string(),
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
}
