//! Dry-run routing: what an event would do, recipient by recipient.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{NotificationRuleId, UserId};

use super::attention::{Attention, AttentionReason};
use super::event::NotificationEvent;
use super::route::RoutePlan;

/// An event to route without persisting or delivering anything.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize, JsonSchema)]
pub struct SimulateRequest {
    pub event: NotificationEvent,
}

/// The router plan plus the final per-recipient outcome.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SimulateResponse {
    pub plan: RoutePlan,
    pub decisions: Vec<SimulatedRecipient>,
}

/// One recipient's simulated outcome; `attention: None` means withheld.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct SimulatedRecipient {
    pub user: UserId,
    pub attention: Option<Attention>,
    pub reason: AttentionReason,
    pub matched_rules: Vec<NotificationRuleId>,
    pub withheld_reason: Option<String>,
}
