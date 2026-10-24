//! File-storage composition: the profile's `storage:` section resolved to a
//! [`FileStorage`] backend, with the credential sources the object-store
//! backends need.
//!
//! The storage crate stays free of the security crate, so the key-file token
//! source ([`ServiceAccountTokens`]) lives here and wraps
//! `systemprompt_security::google::access_token`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;
use std::time::Duration;

use async_trait::async_trait;
use systemprompt_config::paths::AppPaths;
use systemprompt_identifiers::{InstanceId, SecretName};
use systemprompt_manifest::Secrets;
use systemprompt_manifest::profile::{GcsCredentials, StorageBackend, StorageConfig};
use systemprompt_security::credential::TokenCache;
use systemprompt_security::google::{ServiceAccountKey, access_token};
use systemprompt_storage::{
    DEFAULT_METADATA_TOKEN_URL, FileStorageBackend, GcsError, GcsParams, GcsTokenSource,
    MetadataServerTokens, S3Credentials, S3Params,
};
use systemprompt_traits::FileStorage;

use crate::error::{RuntimeError, RuntimeResult};

const OBJECT_STORE_CONNECT_TIMEOUT: Duration = Duration::from_secs(10);
const OBJECT_STORE_READ_TIMEOUT: Duration = Duration::from_secs(60);

/// Cloud Storage tokens minted from a service-account key held in secrets.
#[derive(Debug)]
pub struct ServiceAccountTokens {
    cache_key: String,
    key: ServiceAccountKey,
    tokens: TokenCache,
}

impl ServiceAccountTokens {
    #[must_use]
    pub fn new(secret: &SecretName, key: ServiceAccountKey) -> Self {
        Self {
            cache_key: format!("storage-gcs:{secret}"),
            key,
            tokens: TokenCache::new(),
        }
    }
}

#[async_trait]
impl GcsTokenSource for ServiceAccountTokens {
    async fn bearer(&self) -> Result<String, GcsError> {
        access_token(&self.tokens, &self.cache_key, &self.key)
            .await
            .map_err(|e| GcsError::Token(Box::new(e)))
    }
}

pub(crate) async fn init_file_storage(
    storage: &StorageConfig,
    app_paths: &AppPaths,
    instance_id: &InstanceId,
    secrets: &Secrets,
) -> RuntimeResult<Arc<dyn FileStorage>> {
    match storage.backend {
        StorageBackend::Local => init_local(storage, app_paths, instance_id).await,
        StorageBackend::Gcs => init_gcs(storage, secrets),
        StorageBackend::S3 => init_s3(storage, secrets),
    }
}

async fn init_local(
    storage: &StorageConfig,
    app_paths: &AppPaths,
    instance_id: &InstanceId,
) -> RuntimeResult<Arc<dyn FileStorage>> {
    let root = app_paths.storage().root();
    let report = systemprompt_storage::probe_shared_mount(root, instance_id)
        .await
        .map_err(|source| RuntimeError::StorageProbe {
            path: root.to_path_buf(),
            source,
        })?;
    if !report.write_read_ok {
        return Err(RuntimeError::StorageReadBack {
            path: root.to_path_buf(),
        });
    }
    match (storage.shared, report.has_siblings()) {
        (true, false) => tracing::warn!(
            root = %root.display(),
            "storage.shared is true but no other replica has marked this root; \
             it may be a per-node disk"
        ),
        (false, true) => tracing::warn!(
            root = %root.display(),
            instances = ?report.instances,
            "storage.shared is false but other replicas have marked this root; \
             set storage.shared: true if it is a shared mount"
        ),
        _ => {},
    }
    Ok(systemprompt_storage::build_file_storage(
        FileStorageBackend::Local {
            root: root.to_path_buf(),
        },
    ))
}

fn init_gcs(storage: &StorageConfig, secrets: &Secrets) -> RuntimeResult<Arc<dyn FileStorage>> {
    let http = object_store_http()?;
    let tokens: Arc<dyn GcsTokenSource> = match storage.credentials.clone().unwrap_or_default() {
        GcsCredentials::WorkloadIdentity => {
            let endpoint = url::Url::parse(DEFAULT_METADATA_TOKEN_URL)
                .map_err(RuntimeError::StorageEndpoint)?;
            Arc::new(MetadataServerTokens::new(endpoint, http.clone()))
        },
        GcsCredentials::Secret(name) => {
            let raw = secrets
                .get(name.as_str())
                .ok_or_else(|| RuntimeError::StorageCredentialMissing { name: name.clone() })?;
            let key = serde_json::from_str::<ServiceAccountKey>(raw).map_err(|source| {
                RuntimeError::StorageCredential {
                    name: name.clone(),
                    source,
                }
            })?;
            Arc::new(ServiceAccountTokens::new(&name, key))
        },
    };
    let bucket = required_bucket(storage)?;
    let params = GcsParams::new(bucket, storage.prefix.clone(), storage.public_read)
        .map_err(RuntimeError::StorageEndpoint)?;
    tracing::info!(
        bucket = %params.bucket,
        prefix = params.prefix.as_deref().unwrap_or(""),
        "file storage: Cloud Storage backend"
    );
    Ok(systemprompt_storage::build_file_storage(
        FileStorageBackend::Gcs {
            params: Box::new(params),
            tokens,
            http,
        },
    ))
}

// Why: `/files` streams object bodies straight to the client, so a total
// request deadline would cut a large or slowly-consumed download mid-body
// after its Content-Length was sent; only connect and per-read stalls are
// bounded.
fn object_store_http() -> RuntimeResult<reqwest::Client> {
    reqwest::Client::builder()
        .connect_timeout(OBJECT_STORE_CONNECT_TIMEOUT)
        .read_timeout(OBJECT_STORE_READ_TIMEOUT)
        .build()
        .map_err(RuntimeError::StorageHttp)
}

fn required_bucket(storage: &StorageConfig) -> RuntimeResult<String> {
    storage
        .bucket
        .clone()
        .filter(|bucket| !bucket.is_empty())
        .ok_or(RuntimeError::StorageBucketMissing)
}

fn init_s3(storage: &StorageConfig, secrets: &Secrets) -> RuntimeResult<Arc<dyn FileStorage>> {
    let http = object_store_http()?;
    let Some(GcsCredentials::Secret(name)) = storage.credentials.clone() else {
        return Err(RuntimeError::StorageS3CredentialsRequired);
    };
    let raw = secrets
        .get(name.as_str())
        .ok_or_else(|| RuntimeError::StorageCredentialMissing { name: name.clone() })?;
    let credentials = serde_json::from_str::<S3Credentials>(raw)
        .map_err(|source| RuntimeError::StorageCredential { name, source })?;
    let params = S3Params::new(
        required_bucket(storage)?,
        storage.region.clone(),
        storage.endpoint.as_deref(),
    )
    .map_err(RuntimeError::StorageEndpoint)?
    .with_prefix(storage.prefix.clone())
    .with_path_style(storage.path_style)
    .with_public_read(storage.public_read);
    tracing::info!(
        bucket = %params.bucket,
        prefix = params.prefix.as_deref().unwrap_or(""),
        endpoint = %params.endpoint,
        path_style = params.path_style,
        "file storage: S3-compatible backend"
    );
    Ok(systemprompt_storage::build_file_storage(
        FileStorageBackend::S3 {
            params: Box::new(params),
            credentials,
            http,
        },
    ))
}
