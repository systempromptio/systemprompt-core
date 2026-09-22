//! How long the high-volume operational tables keep their rows.
//!
//! One place, on the profile, for every retention window the scheduler's
//! `database_cleanup` job enforces. Every field has a default, so a profile
//! that says nothing gets bounded tables; a profile that names a window gets
//! exactly that window. Days, never "forever": a table with no deletion
//! path is how a self-hosted instance ran out of memory rebuilding its
//! analytics baseline.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
#[expect(
    clippy::struct_field_names,
    reason = "every field is a window in days; the unit belongs in the name"
)]
pub struct RetentionConfig {
    /// `logs` rows older than this are deleted.
    #[serde(default = "default_logs_days")]
    pub logs_days: u32,

    /// `analytics_events` rows older than this are deleted.
    #[serde(default = "default_analytics_events_days")]
    pub analytics_events_days: u32,

    /// Stored AI request messages (`ai_request_messages`) older than this are
    /// deleted; the request row itself stays. `None` follows
    /// `ai.history.retention_days` from the services configuration.
    #[serde(default)]
    pub ai_request_messages_days: Option<u32>,

    /// `mcp_tool_executions` rows older than this are deleted.
    #[serde(default = "default_mcp_tool_executions_days")]
    pub mcp_tool_executions_days: u32,

    /// Processed `event_outbox` rows older than this are deleted.
    #[serde(default = "default_outbox_processed_days")]
    pub outbox_processed_days: u32,

    /// Raw request and response bodies on `ai_request_payloads` older than
    /// this are set to NULL; the excerpts, hashes and sizes stay.
    #[serde(default = "default_ai_request_payload_raw_days")]
    pub ai_request_payload_raw_days: u32,

    /// `governance_decisions` rows older than this are deleted.
    #[serde(default = "default_governance_decisions_days")]
    pub governance_decisions_days: u32,
}

impl Default for RetentionConfig {
    fn default() -> Self {
        Self {
            logs_days: default_logs_days(),
            analytics_events_days: default_analytics_events_days(),
            ai_request_messages_days: None,
            mcp_tool_executions_days: default_mcp_tool_executions_days(),
            outbox_processed_days: default_outbox_processed_days(),
            ai_request_payload_raw_days: default_ai_request_payload_raw_days(),
            governance_decisions_days: default_governance_decisions_days(),
        }
    }
}

const fn default_logs_days() -> u32 {
    30
}

const fn default_analytics_events_days() -> u32 {
    90
}

const fn default_mcp_tool_executions_days() -> u32 {
    365
}

const fn default_outbox_processed_days() -> u32 {
    7
}

const fn default_ai_request_payload_raw_days() -> u32 {
    7
}

const fn default_governance_decisions_days() -> u32 {
    180
}
