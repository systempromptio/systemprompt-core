//! `storage:` validation: backend-specific keys and the object-store
//! bucket/prefix shape.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{GcsCredentials, Profile, StorageBackend, StorageConfig};

impl Profile {
    pub(crate) fn validate_storage(&self, errors: &mut Vec<String>) {
        if self.paths.storage.as_deref().is_none_or(str::is_empty) {
            errors.push("storage requires paths.storage to be set".to_owned());
        }
        let storage = &self.storage;
        if self.paths.data.as_deref().is_some_and(str::is_empty) {
            errors.push(
                "paths.data must not be empty; omit it to use <paths.storage>/data".to_owned(),
            );
        }
        match storage.backend {
            StorageBackend::Local => {
                if storage.shared && self.paths.data.is_none() {
                    errors.push(
                        "storage.shared puts paths.storage on a mount every replica sees; set \
                         paths.data to a per-node writable volume so each node's gateway \
                         journal and scratch stay its own"
                            .to_owned(),
                    );
                }
                for (key, set) in [
                    ("bucket", storage.bucket.is_some()),
                    ("prefix", storage.prefix.is_some()),
                    ("public_read", storage.public_read),
                    ("credentials", storage.credentials.is_some()),
                ] {
                    if set {
                        errors.push(format!(
                            "storage.{key} applies to backends 'gcs' and 's3' only; remove it \
                             or set storage.backend"
                        ));
                    }
                }
                reject_s3_keys(storage, "local", errors);
            },
            StorageBackend::Gcs => {
                validate_object_store(storage, "gcs", errors);
                reject_s3_keys(storage, "gcs", errors);
            },
            StorageBackend::S3 => {
                validate_object_store(storage, "s3", errors);
                validate_s3(storage, errors);
            },
        }
    }
}

fn validate_s3(storage: &StorageConfig, errors: &mut Vec<String>) {
    if !storage.path_style
        && let Some(bucket) = storage.bucket.as_deref()
        && bucket.contains(['_', '.'])
    {
        errors.push(format!(
            "storage.bucket '{bucket}' contains '_' or '.', which cannot be addressed as a \
             virtual-hosted S3 bucket (invalid hostname or TLS certificate mismatch); set \
             storage.path_style: true"
        ));
    }
    if !matches!(storage.credentials, Some(GcsCredentials::Secret(_))) {
        errors
            .push("storage.backend 's3' requires storage.credentials: { secret: NAME }".to_owned());
    }
    if let Some(endpoint) = storage.endpoint.as_deref()
        && !(endpoint.starts_with("https://") || endpoint.starts_with("http://"))
    {
        errors.push(format!(
            "storage.endpoint '{endpoint}' must be an http:// or https:// URL"
        ));
    }
    if storage
        .region
        .as_deref()
        .is_some_and(|r| r.trim().is_empty())
    {
        errors.push("storage.region must not be empty".to_owned());
    }
}

fn reject_s3_keys(storage: &StorageConfig, backend: &str, errors: &mut Vec<String>) {
    for (key, set) in [
        ("endpoint", storage.endpoint.is_some()),
        ("region", storage.region.is_some()),
        ("path_style", storage.path_style),
    ] {
        if set {
            errors.push(format!(
                "storage.{key} applies to backend 's3' only, not '{backend}'"
            ));
        }
    }
}

fn validate_object_store(storage: &StorageConfig, backend: &str, errors: &mut Vec<String>) {
    match storage.bucket.as_deref() {
        None | Some("") => {
            errors.push(format!(
                "storage.backend '{backend}' requires storage.bucket"
            ));
        },
        Some(bucket) if !is_bucket_name(bucket) => errors.push(format!(
            "storage.bucket '{bucket}' is not a valid {} bucket name (3-222 chars of a-z, 0-9, \
             '.', '_', '-', starting and ending with a letter or digit)",
            if backend == "gcs" {
                "Cloud Storage"
            } else {
                "S3"
            }
        )),
        Some(_) => {},
    }
    if let Some(prefix) = storage.prefix.as_deref()
        && !is_object_prefix(prefix)
    {
        errors.push(format!(
            "storage.prefix '{prefix}' must be non-empty, must not start or end with '/', and \
             must not contain empty, '.' or '..' segments"
        ));
    }
    if storage.shared {
        errors.push(
            "storage.shared applies to backend 'local' only; a bucket is shared by every replica"
                .to_owned(),
        );
    }
    if let Some(GcsCredentials::Secret(name)) = &storage.credentials
        && name.as_str().trim().is_empty()
    {
        errors.push("storage.credentials.secret must name a secret".to_owned());
    }
}

fn is_bucket_name(name: &str) -> bool {
    let bytes = name.as_bytes();
    let edge = |b: &u8| b.is_ascii_lowercase() || b.is_ascii_digit();
    (3..=222).contains(&bytes.len())
        && bytes.first().is_some_and(edge)
        && bytes.last().is_some_and(edge)
        && bytes
            .iter()
            .all(|b| edge(b) || matches!(b, b'.' | b'_' | b'-'))
}

fn is_object_prefix(prefix: &str) -> bool {
    !prefix.is_empty()
        && prefix
            .split('/')
            .all(|segment| !segment.is_empty() && segment != "." && segment != "..")
}
