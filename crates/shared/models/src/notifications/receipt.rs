//! Delivery and read receipts per surface.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::str::FromStr;

use chrono::{DateTime, Utc};
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::NotificationId;

use super::attention::UnknownNotificationValue;

pub const SURFACE_AGUI: &str = "agui";
pub const SURFACE_DASHBOARD: &str = "dashboard";
pub const SURFACE_BRIDGE: &str = "bridge";
pub const SURFACE_MCP: &str = "mcp";
pub const SURFACE_API: &str = "api";

/// How far a notification got on one surface.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize, JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum ReceiptState {
    Delivered,
    Seen,
    Read,
    Acted,
    Escalated,
}

impl ReceiptState {
    #[must_use]
    pub const fn as_str(self) -> &'static str {
        match self {
            Self::Delivered => "delivered",
            Self::Seen => "seen",
            Self::Read => "read",
            Self::Acted => "acted",
            Self::Escalated => "escalated",
        }
    }
}

impl fmt::Display for ReceiptState {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(self.as_str())
    }
}

impl FromStr for ReceiptState {
    type Err = UnknownNotificationValue;

    fn from_str(s: &str) -> Result<Self, Self::Err> {
        match s {
            "delivered" => Ok(Self::Delivered),
            "seen" => Ok(Self::Seen),
            "read" => Ok(Self::Read),
            "acted" => Ok(Self::Acted),
            "escalated" => Ok(Self::Escalated),
            other => Err(UnknownNotificationValue::new("receipt_state", other)),
        }
    }
}

/// One receipt: a notification reached a state on a surface at a time.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct Receipt {
    pub notification_id: NotificationId,
    pub surface: String,
    pub state: ReceiptState,
    pub at: DateTime<Utc>,
}
