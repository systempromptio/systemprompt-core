//! Gateway endpoints describing *who this bridge is* and what its plan allows:
//! whoami, the bridge profile, token usage, governance decisions, and the
//! per-host model filter. The artefact endpoints — pubkey, signed manifest,
//! plugin files and releases — live in `fetch.rs`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::ids::BearerToken;
use std::time::Instant;

use systemprompt_models::api::cloud::BridgeProfileUsage;
use systemprompt_models::bridge::host::HostKind;

use crate::gateway::errors::GatewayError;
use crate::gateway::identity_source::whoami_path;
use crate::gateway::types::{BridgeProfile, SelfEnrollRequest, SelfEnrollResponse, WhoamiResponse};
use crate::gateway::{GatewayClient, ensure_success, record_span};

#[derive(serde::Serialize)]
struct HostModelFilterRequest<'a> {
    host_id: HostKind,
    model_protocols: Option<&'a [String]>,
}

impl GatewayClient {
    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "whoami", status, latency_ms)
    )]
    pub async fn fetch_whoami(&self, bearer: &BearerToken) -> Result<WhoamiResponse, GatewayError> {
        let url = self.url(whoami_path());
        let started = Instant::now();
        let resp = self
            .http()
            .get(&url)
            .bearer_auth(bearer.expose())
            .send()
            .await
            .map_err(|e| GatewayError::WhoamiFetch(Box::new(e)))?;
        record_span(&resp, started);
        let resp = ensure_success(resp, "whoami").await?;
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
        bearer: &BearerToken,
        host_id: HostKind,
        protocols: Option<&[String]>,
    ) -> Result<(), GatewayError> {
        let url = self.url("/v1/bridge/profile/host-model-filter");
        let body = HostModelFilterRequest {
            host_id,
            model_protocols: protocols,
        };
        let started = Instant::now();
        let resp = self
            .http()
            .post(&url)
            .bearer_auth(bearer.expose())
            .json(&body)
            .send()
            .await
            .map_err(|e| GatewayError::PostRequest(Box::new(e)))?;
        record_span(&resp, started);
        ensure_success(resp, "host-model-filter").await?;
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
        let resp = ensure_success(resp, "profile").await?;
        let profile = resp
            .json::<BridgeProfile>()
            .await
            .map_err(|e| GatewayError::ProfileDecode(Box::new(e)))?;
        if let Err(error) =
            crate::install::mdm::desktop_catalog::remember(self.base_url().as_str(), &profile)
        {
            tracing::warn!(%error, "could not cache the desktop model catalog");
        }
        Ok(profile)
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "profile_usage", status, latency_ms)
    )]
    pub async fn fetch_profile_usage(
        &self,
        bearer: &BearerToken,
    ) -> Result<BridgeProfileUsage, GatewayError> {
        let url = self.url("/v1/bridge/profile/usage");
        let started = Instant::now();
        let resp = self
            .http()
            .get(&url)
            .bearer_auth(bearer.expose())
            .send()
            .await
            .map_err(|e| GatewayError::ProfileUsageFetch(Box::new(e)))?;
        record_span(&resp, started);
        let resp = ensure_success(resp, "profile_usage").await?;
        resp.json::<BridgeProfileUsage>()
            .await
            .map_err(|e| GatewayError::ProfileUsageDecode(Box::new(e)))
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer, request),
        fields(endpoint = "device", status, latency_ms)
    )]
    pub async fn enroll_device(
        &self,
        bearer: &BearerToken,
        request: &SelfEnrollRequest,
    ) -> Result<SelfEnrollResponse, GatewayError> {
        let url = self.url("/v1/bridge/device");
        let started = Instant::now();
        let resp = self
            .http()
            .post(&url)
            .bearer_auth(bearer.expose())
            .json(request)
            .send()
            .await
            .map_err(|e| GatewayError::PostRequest(Box::new(e)))?;
        record_span(&resp, started);
        let resp = ensure_success(resp, "device").await?;
        resp.json::<SelfEnrollResponse>()
            .await
            .map_err(|e| GatewayError::DeviceEnrollDecode(Box::new(e)))
    }
}
