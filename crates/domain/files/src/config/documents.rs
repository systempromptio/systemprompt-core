//! The `files.documents` section: the document store's on/off switch
//! (off by default), upload limit, blob
//! garbage-collection grace, presence time-to-live, trash retention, the
//! retention of the agent read audit (`agentReadRetentionDays`),
//! per-request input limits, the resumable upload limits (`uploads`) and
//! whether a user with no access may request it
//! (`accessRequests`, off by default), and the live-session tuning
//! (`collab`: snapshot cadence and the owner lease).
//!
//! Turning `accessRequests` on tells a signed-in caller that a document they
//! cannot open exists (the refusal carries `can_request: true`); with it off
//! such a document is indistinguishable from a missing one.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

use super::document_limits::DocumentLimits;
use super::upload_limits::UploadLimits;

pub const MIN_DOCUMENT_UPLOAD_BYTES: u64 = 1024 * 1024;
pub const MAX_DOCUMENT_UPLOAD_BYTES: u64 = 2 * 1024 * 1024 * 1024;
const DEFAULT_MAX_UPLOAD_BYTES: u64 = 64 * 1024 * 1024;
const DEFAULT_BLOB_GC_GRACE_HOURS: u32 = 24;
const DEFAULT_PRESENCE_TTL_SECS: u32 = 90;
const DEFAULT_TRASH_RETENTION_DAYS: u32 = 30;
const DEFAULT_AGENT_READ_RETENTION_DAYS: u32 = 90;
const DEFAULT_SNAPSHOT_AFTER_SECS: u32 = 30;
const DEFAULT_SNAPSHOT_AFTER_UPDATES: u32 = 200;
const DEFAULT_OWNER_PORT: u16 = 8080;

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
    #[serde(default = "default_agent_read_retention_days")]
    pub agent_read_retention_days: u32,
    #[serde(default)]
    pub limits: DocumentLimits,
    #[serde(default)]
    pub uploads: UploadLimits,
    #[serde(default)]
    pub access_requests: bool,
    #[serde(default)]
    pub collab: CollabConfig,
}

/// When a live session snapshots, and whether this replica takes part in the
/// owner lease. A replica's advertised address is per machine, so only the
/// port is configured; the host comes from the platform at boot.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CollabConfig {
    #[serde(default = "default_snapshot_after_secs")]
    pub snapshot_after_secs: u32,
    #[serde(default = "default_snapshot_after_updates")]
    pub snapshot_after_updates: u32,
    #[serde(default)]
    pub owner: CollabOwnerConfig,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", deny_unknown_fields)]
pub struct CollabOwnerConfig {
    #[serde(default)]
    pub enabled: bool,
    #[serde(default = "default_owner_port")]
    pub advertise_port: u16,
}

const fn default_snapshot_after_secs() -> u32 {
    DEFAULT_SNAPSHOT_AFTER_SECS
}

const fn default_snapshot_after_updates() -> u32 {
    DEFAULT_SNAPSHOT_AFTER_UPDATES
}

const fn default_owner_port() -> u16 {
    DEFAULT_OWNER_PORT
}

impl Default for CollabConfig {
    fn default() -> Self {
        Self {
            snapshot_after_secs: DEFAULT_SNAPSHOT_AFTER_SECS,
            snapshot_after_updates: DEFAULT_SNAPSHOT_AFTER_UPDATES,
            owner: CollabOwnerConfig::default(),
        }
    }
}

impl Default for CollabOwnerConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            advertise_port: DEFAULT_OWNER_PORT,
        }
    }
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

const fn default_agent_read_retention_days() -> u32 {
    DEFAULT_AGENT_READ_RETENTION_DAYS
}

impl Default for DocumentsConfig {
    fn default() -> Self {
        Self {
            enabled: false,
            max_upload_bytes: DEFAULT_MAX_UPLOAD_BYTES,
            blob_gc_grace_hours: DEFAULT_BLOB_GC_GRACE_HOURS,
            presence_ttl_secs: DEFAULT_PRESENCE_TTL_SECS,
            trash_retention_days: DEFAULT_TRASH_RETENTION_DAYS,
            agent_read_retention_days: DEFAULT_AGENT_READ_RETENTION_DAYS,
            limits: DocumentLimits::default(),
            uploads: UploadLimits::default(),
            access_requests: false,
            collab: CollabConfig::default(),
        }
    }
}

impl DocumentsConfig {
    #[must_use]
    pub const fn is_enabled(&self) -> bool {
        self.enabled
    }
}
