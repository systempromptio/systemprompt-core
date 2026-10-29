//! Notification pipeline settings: feed, unread and failure retention, the
//! largest audience one subject may expand to, and the outbox consumer's
//! batch, cadence and per-call timeouts.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// The `notifications:` profile block; every field has a default.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NotificationsProfile {
    #[serde(default = "default_retention_days")]
    pub retention_days: u32,
    #[serde(default = "default_max_audience")]
    pub max_audience: u32,
    #[serde(default = "default_unread_max_age_days")]
    pub unread_max_age_days: u32,
    #[serde(default = "default_failure_retention_days")]
    pub failure_retention_days: u32,
    #[serde(default)]
    pub consumer: ConsumerProfile,
}

/// How the notifications outbox consumer drains.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ConsumerProfile {
    #[serde(default = "default_batch")]
    pub batch: u32,
    #[serde(default = "default_poll_secs")]
    pub poll_secs: u32,
    #[serde(default = "default_sweep_secs")]
    pub sweep_secs: u32,
    #[serde(default = "default_timeout_ms")]
    pub resolver_timeout_ms: u64,
    #[serde(default = "default_timeout_ms")]
    pub channel_timeout_ms: u64,
}

const fn default_retention_days() -> u32 {
    90
}

const fn default_max_audience() -> u32 {
    500
}

const fn default_unread_max_age_days() -> u32 {
    180
}

const fn default_failure_retention_days() -> u32 {
    30
}

const fn default_batch() -> u32 {
    64
}

const fn default_poll_secs() -> u32 {
    5
}

const fn default_sweep_secs() -> u32 {
    30
}

const fn default_timeout_ms() -> u64 {
    5000
}

impl Default for NotificationsProfile {
    fn default() -> Self {
        Self {
            retention_days: default_retention_days(),
            max_audience: default_max_audience(),
            unread_max_age_days: default_unread_max_age_days(),
            failure_retention_days: default_failure_retention_days(),
            consumer: ConsumerProfile::default(),
        }
    }
}

impl Default for ConsumerProfile {
    fn default() -> Self {
        Self {
            batch: default_batch(),
            poll_secs: default_poll_secs(),
            sweep_secs: default_sweep_secs(),
            resolver_timeout_ms: default_timeout_ms(),
            channel_timeout_ms: default_timeout_ms(),
        }
    }
}
