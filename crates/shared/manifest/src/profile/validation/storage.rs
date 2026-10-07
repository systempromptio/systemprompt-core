//! `storage:` validation: backend-specific keys and the GCS bucket/prefix
//! shape.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use super::super::{Profile, StorageBackend};

impl Profile {
    pub(crate) fn validate_storage(&self, errors: &mut Vec<String>) {
        if self.paths.storage.as_deref().is_none_or(str::is_empty) {
            errors.push("storage requires paths.storage to be set".to_owned());
        }
        let storage = &self.storage;
        match storage.backend {
            StorageBackend::Local => {
                for (key, set) in [
                    ("bucket", storage.bucket.is_some()),
                    ("prefix", storage.prefix.is_some()),
                    ("public_read", storage.public_read),
                    ("credentials", storage.credentials.is_some()),
                ] {
                    if set {
                        errors.push(format!(
                            "storage.{key} applies to backend 'gcs' only; remove it or set \
                             storage.backend: gcs"
                        ));
                    }
                }
            },
            StorageBackend::Gcs => {
                match storage.bucket.as_deref() {
                    None | Some("") => {
                        errors.push("storage.backend 'gcs' requires storage.bucket".to_owned());
                    },
                    Some(bucket) if !is_bucket_name(bucket) => errors.push(format!(
                        "storage.bucket '{bucket}' is not a valid Cloud Storage bucket name \
                         (3-222 chars of a-z, 0-9, '.', '_', '-', starting and ending with a \
                         letter or digit)"
                    )),
                    Some(_) => {},
                }
                if let Some(prefix) = storage.prefix.as_deref()
                    && !is_object_prefix(prefix)
                {
                    errors.push(format!(
                        "storage.prefix '{prefix}' must be non-empty, must not start or end \
                         with '/', and must not contain empty, '.' or '..' segments"
                    ));
                }
                if storage.shared {
                    errors.push(
                        "storage.shared applies to backend 'local' only; a bucket is shared by \
                         every replica"
                            .to_owned(),
                    );
                }
                if let Some(super::super::GcsCredentials::Secret(name)) = &storage.credentials
                    && name.as_str().trim().is_empty()
                {
                    errors.push("storage.credentials.secret must name a secret".to_owned());
                }
            },
        }
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
