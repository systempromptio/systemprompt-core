//! The `files.documents` section: the document store's on/off switch
//! (off by default), upload limit, blob
//! garbage-collection grace, presence time-to-live, trash retention and
//! per-request input limits.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::document_limits::DocumentLimits;

pub const MIN_DOCUMENT_UPLOAD_BYTES: u64 = 1024 * 1024;
pub const MAX_DOCUMENT_UPLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DEFAULT_MAX_UPLOAD_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_BLOB_GC_GRACE_HOURS: u32 = 24;
const DEFAULT_PRESENCE_TTL_SECS: u32 = 90;
const DEFAULT_TRASH_RETENTION_DAYS: u32 = 30;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct DocumentsConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_max_upload_bytes")]
    pub max_upload_bytes: u64,
    #[serde(default = "default_blob_gc_grace_hours")]
    pub blob_gc_grace_hours: u32,
    #[serde(default = "default_presence_ttl_secs")]
    pub presence_ttl_secs: u32,
    #[serde(default = "default_trash_retention_days")]
    pub trash_retention_days: u32,
    #[serde(default)]
    pub limits: DocumentLimits,
}

const fn default_max_upload_bytes() -> u64 {
    DEFAULT_MAX_UPLOAD_BYTES
}

const fn default_blob_gc_grace_hours() -> u32 {
    DEFAULT_BLOB_GC_GRACE_HOURS
}

const fn default_presence_ttl_secs() -> u32 {
    DEFAULT_PRESENCE_TTL_SECS
}

const fn default_trash_retention_days() -> u32 {
    DEFAULT_TRASH_RETENTION_DAYS
}

impl Default for DocumentsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_upload_bytes: DEFAULT_MAX_UPLOAD_BYTES,
            blob_gc_grace_hours: DEFAULT_BLOB_GC_GRACE_HOURS,
            presence_ttl_secs: DEFAULT_PRESENCE_TTL_SECS,
            trash_retention_days: DEFAULT_TRASH_RETENTION_DAYS,
            limits: DocumentLimits::default(),
        }
    }
}

impl DocumentsConfig {
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }
}
