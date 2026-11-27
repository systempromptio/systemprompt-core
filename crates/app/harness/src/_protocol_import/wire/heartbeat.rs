//! The heartbeat.
//!
//! `POST /v1/bridge/heartbeat` with the user's JWT, every 30 seconds while
//! signed in: the instance lists the device as online and answers whether this
//! client's protocol version is still supported.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};

use crate::ids::SessionId;

pub const HEARTBEAT_INTERVAL_SECS: u64 = 30;

/// Whether the client is serving its user or switched off by them.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum ClientState {
    Active,
    Disabled,
}

/// The heartbeat body.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatRequest {
    pub session_id: SessionId,
    pub bridge_version: String,
    pub os: String,
    pub hostname: String,
    pub last_activity_at: Option<DateTime<Utc>>,
    pub forwarded_total: i64,
    pub tokens_in_total: i64,
    pub tokens_out_total: i64,
    pub release_version: String,
    pub state: ClientState,
}

/// Where this client's release sits in the window the instance admits.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum WindowStatus {
    Below,
    Within,
    Above,
    #[serde(other)]
    Unknown,
}

/// The heartbeat answer.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct HeartbeatResponse {
    pub min_bridge_version: String,
    pub compatible: bool,
    #[serde(default)]
    pub window_status: Option<WindowStatus>,
}

impl HeartbeatResponse {
    #[must_use]
    pub const fn unsupported(&self) -> bool {
        !self.compatible
            || matches!(
                self.window_status,
                Some(WindowStatus::Below | WindowStatus::Above)
            )
    }
}

#[must_use]
pub fn saturating_total(count: u64) -> i64 {
    i64::try_from(count).unwrap_or(i64::MAX)
}
