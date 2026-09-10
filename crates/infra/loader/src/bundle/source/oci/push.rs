//! Publishing a bundle to an OCI registry.
//!
//! Uploads are monolithic: a session is opened, the whole blob is PUT with
//! its digest, and the registry's own digest check is the acceptance test.
//! A push whose response the registry does not accept is an error — a
//! publish that "mostly worked" would leave a manifest pointing at bytes no
//! instance can pull.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::path::Path;

use sha2::{Digest, Sha256};
use systemprompt_models::services::bundle::BUNDLE_MEDIA_TYPE;

use super::RegistryClient;
use super::pull::{OCI_MANIFEST_MEDIA_TYPE, OciDescriptor, OciManifest};
use crate::bundle::error::{BundleError, BundleResult};

pub const BUNDLE_CONFIG_MEDIA_TYPE: &str =
    "application/vnd.systemprompt.services-bundle.config.v1+json";

pub async fn push_bundle(
    reference: &str,
    archive: &Path,
    manifest_json: &[u8],
    secret: Option<String>,
    client: reqwest::Client,
) -> BundleResult<String> {
    let registry = RegistryClient::new("publish", reference, secret, client)?;
    let archive_bytes = tokio::fs::read(archive).await?;

    let config = upload_blob(&registry, manifest_json, BUNDLE_CONFIG_MEDIA_TYPE).await?;
    let layer = upload_blob(&registry, &archive_bytes, BUNDLE_MEDIA_TYPE).await?;

    let manifest = OciManifest {
        schema_version: 2,
        media_type: Some(OCI_MANIFEST_MEDIA_TYPE.to_owned()),
        artifact_type: Some(BUNDLE_MEDIA_TYPE.to_owned()),
        config,
        layers: vec![layer],
    };
    put_manifest(&registry, &manifest).await
}

async fn upload_blob(
    registry: &RegistryClient,
    body: &[u8],
    media_type: &str,
) -> BundleResult<OciDescriptor> {
    let digest = format!("sha256:{}", hex::encode(Sha256::digest(body)));

    let initiate_url = registry.url("/blobs/uploads/")?;
    let initiated = registry
        .send(move |client| client.post(initiate_url.clone()))
        .await?;
    if !initiated.status().is_success() {
        return Err(BundleError::fetch(
            &registry.name,
            format!("upload session refused: {}", initiated.status()),
        ));
    }
    let location = initiated
        .headers()
        .get(reqwest::header::LOCATION)
        .and_then(|v| v.to_str().ok())
        .ok_or_else(|| BundleError::fetch(&registry.name, "upload session has no Location"))?
        .to_owned();
    let upload_url = absolute_location(registry, &location)?;

    let owned = body.to_vec();
    let digest_param = digest.clone();
    let response = registry
        .send(move |client| {
            client
                .put(upload_url.clone())
                .query(&[("digest", digest_param.as_str())])
                .header(reqwest::header::CONTENT_TYPE, "application/octet-stream")
                .body(owned.clone())
        })
        .await?;
    if !response.status().is_success() {
        return Err(BundleError::fetch(
            &registry.name,
            format!("blob upload failed: {}", response.status()),
        ));
    }

    Ok(OciDescriptor {
        media_type: media_type.to_owned(),
        digest,
        size: body.len() as u64,
    })
}

fn absolute_location(registry: &RegistryClient, location: &str) -> BundleResult<url::Url> {
    if location.starts_with("http://") || location.starts_with("https://") {
        return url::Url::parse(location)
            .map_err(|e| BundleError::fetch(&registry.name, format!("bad upload location: {e}")));
    }
    let base = registry.url("")?;
    base.join(location)
        .map_err(|e| BundleError::fetch(&registry.name, format!("bad upload location: {e}")))
}

async fn put_manifest(registry: &RegistryClient, manifest: &OciManifest) -> BundleResult<String> {
    let body = serde_json::to_vec(manifest)
        .map_err(|e| BundleError::policy(format!("manifest is not serialisable: {e}")))?;
    let digest = format!("sha256:{}", hex::encode(Sha256::digest(&body)));
    let url = registry.url(&format!("/manifests/{}", registry.manifest_ref()))?;

    let response = registry
        .send(move |client| {
            client
                .put(url.clone())
                .header(reqwest::header::CONTENT_TYPE, OCI_MANIFEST_MEDIA_TYPE)
                .body(body.clone())
        })
        .await?;
    if !response.status().is_success() {
        return Err(BundleError::fetch(
            &registry.name,
            format!("manifest push failed: {}", response.status()),
        ));
    }
    Ok(digest)
}
