//! Manifest and blob reads against an OCI registry.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use serde::{Deserialize, Serialize};
use sha2::{Digest, Sha256};
use systemprompt_models::services::bundle::BUNDLE_MEDIA_TYPE;

use super::RegistryClient;
use crate::bundle::error::{BundleError, BundleResult};
use crate::bundle::source::FetchedBundle;
use crate::bundle::source::stream::stream_to_file;

pub const OCI_MANIFEST_MEDIA_TYPE: &str = "application/vnd.oci.image.manifest.v1+json";
pub const DOCKER_MANIFEST_MEDIA_TYPE: &str = "application/vnd.docker.distribution.manifest.v2+json";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciDescriptor {
    #[serde(rename = "mediaType")]
    pub media_type: String,

    pub digest: String,

    pub size: u64,
}

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct OciManifest {
    #[serde(rename = "schemaVersion")]
    pub schema_version: u32,

    #[serde(rename = "mediaType", default, skip_serializing_if = "Option::is_none")]
    pub media_type: Option<String>,

    #[serde(
        rename = "artifactType",
        default,
        skip_serializing_if = "Option::is_none"
    )]
    pub artifact_type: Option<String>,

    pub config: OciDescriptor,

    #[serde(default)]
    pub layers: Vec<OciDescriptor>,
}

pub async fn get_manifest(registry: &RegistryClient) -> BundleResult<(String, OciManifest)> {
    let url = registry.url(&format!("/manifests/{}", registry.manifest_ref()))?;
    let accept = format!("{OCI_MANIFEST_MEDIA_TYPE}, {DOCKER_MANIFEST_MEDIA_TYPE}");
    let response = registry
        .send(move |client| {
            client
                .get(url.clone())
                .header(reqwest::header::ACCEPT, accept.clone())
        })
        .await?;

    let status = response.status();
    if !status.is_success() {
        return Err(BundleError::fetch(
            &registry.name,
            format!("manifest request failed: {status}"),
        ));
    }

    let header_digest = response
        .headers()
        .get("docker-content-digest")
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    let body = response
        .bytes()
        .await
        .map_err(|e| BundleError::fetch(&registry.name, e))?;
    let digest =
        header_digest.unwrap_or_else(|| format!("sha256:{}", hex::encode(Sha256::digest(&body))));

    let manifest: OciManifest = serde_json::from_slice(&body)
        .map_err(|e| BundleError::fetch(&registry.name, format!("manifest does not parse: {e}")))?;
    Ok((digest, manifest))
}

pub async fn pull_bundle_layer(
    registry: &RegistryClient,
    manifest: &OciManifest,
    into: &Path,
    max_bytes: u64,
) -> BundleResult<FetchedBundle> {
    let matching: Vec<&OciDescriptor> = manifest
        .layers
        .iter()
        .filter(|l| l.media_type == BUNDLE_MEDIA_TYPE)
        .collect();
    let [layer] = matching.as_slice() else {
        return Err(BundleError::fetch(
            &registry.name,
            format!(
                "manifest carries {} layers of {BUNDLE_MEDIA_TYPE}, expected exactly one",
                matching.len()
            ),
        ));
    };

    if layer.size > max_bytes {
        return Err(BundleError::TooLarge { bytes: max_bytes });
    }

    let url = registry.url(&format!("/blobs/{}", layer.digest))?;
    let response = registry.send(move |client| client.get(url.clone())).await?;
    let status = response.status();
    if !status.is_success() {
        return Err(BundleError::fetch(
            &registry.name,
            format!("blob request failed: {status}"),
        ));
    }

    let digest = stream_to_file(response, into, &registry.name, max_bytes).await?;
    let expected = layer
        .digest
        .strip_prefix("sha256:")
        .unwrap_or(&layer.digest);
    if !digest.eq_ignore_ascii_case(expected) {
        return Err(BundleError::fetch(
            &registry.name,
            format!(
                "blob digest is sha256:{digest}, manifest declares {}",
                layer.digest
            ),
        ));
    }

    Ok(FetchedBundle {
        archive: into.to_path_buf(),
        digest: format!("sha256:{digest}"),
    })
}
