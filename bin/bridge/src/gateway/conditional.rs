//! Revalidating gateway fetches: the manifest under `If-None-Match` and the
//! content-addressed plugin archive.
//!
//! Both answer "nothing new" or "not here" without an error: a 304 tells the
//! caller to reuse the envelope it already verified, and an archive 404 means
//! an older gateway that only serves plugins file by file.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::{Duration, Instant};

use reqwest::StatusCode;

use crate::gateway::errors::GatewayError;
use crate::gateway::fetch::Freshness;
use crate::gateway::manifest::{
    MANIFEST_SCHEMA_HEADER, MANIFEST_SCHEMA_VERSION, SignedManifestEnvelope,
};
use crate::gateway::{GatewayClient, ensure_success, record_span};
use crate::ids::{BearerToken, PluginId, Sha256Digest};

// Why: the archive of a large plugin runs to megabytes over a slow link, so
// the client-wide 30s ceiling fails a healthy download.
const CONDITIONAL_TIMEOUT: Duration = Duration::from_secs(120);

#[derive(Debug)]
pub enum ManifestResponse {
    Fresh {
        envelope: SignedManifestEnvelope,
        etag: Option<String>,
    },
    NotModified,
}

#[derive(Debug)]
pub enum ArchiveResponse {
    Archive(Vec<u8>),
    // Why: The gateway predates the archive route; fetch file by file.
    Unsupported,
    // Why: The gateway no longer holds this plugin digest; the manifest is stale.
    Stale,
}

impl GatewayClient {
    #[tracing::instrument(
        level = "debug",
        skip(self, bearer, if_none_match),
        fields(endpoint = "manifest", status, latency_ms)
    )]
    pub async fn fetch_manifest_conditional(
        &self,
        bearer: &BearerToken,
        freshness: Freshness,
        if_none_match: Option<&str>,
    ) -> Result<ManifestResponse, GatewayError> {
        let url = self.url("/v1/bridge/manifest");
        let started = Instant::now();
        let mut request = self
            .http()
            .get(&url)
            .timeout(CONDITIONAL_TIMEOUT)
            .bearer_auth(bearer.expose())
            .header(MANIFEST_SCHEMA_HEADER, MANIFEST_SCHEMA_VERSION.to_string());
        if freshness == Freshness::Fresh {
            request = request.header(reqwest::header::CACHE_CONTROL, "no-cache");
        }
        if let Some(etag) = if_none_match {
            request = request.header(reqwest::header::IF_NONE_MATCH, etag);
        }
        let resp = request
            .send()
            .await
            .map_err(|e| GatewayError::ManifestFetch(Box::new(e)))?;
        record_span(&resp, started);
        if resp.status() == StatusCode::NOT_MODIFIED && if_none_match.is_some() {
            return Ok(ManifestResponse::NotModified);
        }
        let resp = ensure_success(resp, "manifest").await?;
        let etag = resp
            .headers()
            .get(reqwest::header::ETAG)
            .and_then(|v| v.to_str().ok())
            .map(str::to_owned);
        let body = resp
            .text()
            .await
            .map_err(|e| GatewayError::ManifestDecode(Box::new(e)))?;
        let envelope = serde_json::from_str::<SignedManifestEnvelope>(&body).map_err(|source| {
            GatewayError::ManifestEnvelopeShape {
                snippet: body.chars().take(120).collect(),
                source,
            }
        })?;
        Ok(ManifestResponse::Fresh { envelope, etag })
    }

    #[tracing::instrument(
        level = "debug",
        skip(self, bearer),
        fields(endpoint = "plugin-archive", status, latency_ms)
    )]
    pub async fn fetch_plugin_archive(
        &self,
        bearer: &BearerToken,
        plugin_id: &PluginId,
        sha256: &Sha256Digest,
    ) -> Result<ArchiveResponse, GatewayError> {
        let url = self.url(&format!(
            "/v1/bridge/plugins/{plugin_id}/archive/{}",
            sha256.as_str()
        ));
        let started = Instant::now();
        let fetch_error = |e: reqwest::Error| GatewayError::PluginFetch {
            plugin_id: plugin_id.clone(),
            path: "archive".to_owned(),
            source: Box::new(e),
        };
        let resp = self
            .http()
            .get(&url)
            .timeout(CONDITIONAL_TIMEOUT)
            .bearer_auth(bearer.expose())
            .send()
            .await
            .map_err(fetch_error)?;
        record_span(&resp, started);
        match resp.status() {
            StatusCode::NOT_FOUND => return Ok(ArchiveResponse::Unsupported),
            StatusCode::CONFLICT => return Ok(ArchiveResponse::Stale),
            _ => {},
        }
        let resp = ensure_success(resp, "plugin-archive").await?;
        let bytes = resp.bytes().await.map_err(|e| GatewayError::PluginRead {
            plugin_id: plugin_id.clone(),
            path: "archive".to_owned(),
            source: Box::new(e),
        })?;
        Ok(ArchiveResponse::Archive(bytes.to_vec()))
    }
}
