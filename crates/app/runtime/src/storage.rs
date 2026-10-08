//! File-storage composition: the profile's `storage:` section resolved to a
//! [`FileStorage`] backend, with the credential source the GCS backend needs.
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
use systemprompt_security::google::{ServiceAccountKey, access_token};
use systemprompt_storage::{
    DEFAULT_METADATA_TOKEN_URL, FileStorageBackend, GcsError, GcsParams, GcsTokenSource,
    MetadataServerTokens,
};
use systemprompt_traits::FileStorage;

use crate::error::{RuntimeError, RuntimeResult};

const GCS_REQUEST_TIMEOUT: Duration = Duration::from_secs(60);

/// Cloud Storage tokens minted from a service-account key held in secrets.
#[derive(Debug)]
pub struct ServiceAccountTokens {
    cache_key: String,
    key: ServiceAccountKey,
}

impl ServiceAccountTokens {
    #[must_use]
    pub fn new(secret: &SecretName, key: ServiceAccountKey) -> Self {
        Self {
            cache_key: format!("storage-gcs:{secret}"),
            key,
        }
    }
}

#[async_trait]
impl GcsTokenSource for ServiceAccountTokens {
    async fn bearer(&self) -> Result<String, GcsError> {
        access_token(&self.cache_key, &self.key)
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
    let http = reqwest::Client::builder()
        .timeout(GCS_REQUEST_TIMEOUT)
        .build()
        .map_err(RuntimeError::StorageHttp)?;
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
    let bucket = storage
        .bucket
        .clone()
        .filter(|bucket| !bucket.is_empty())
        .ok_or(RuntimeError::StorageBucketMissing)?;
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
