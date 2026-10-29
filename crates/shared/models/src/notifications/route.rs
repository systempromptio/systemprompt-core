//! A routing decision: who receives an event and at what attention.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{NotificationStreamId, UserId};

use super::attention::{Attention, AttentionReason};

/// One recipient's attention and the reason for it.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RecipientDecision {
    pub user: UserId,
    pub attention: Attention,
    pub reason: AttentionReason,
}

/// The recipients a router chose for one event.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub struct RoutePlan {
    pub recipients: Vec<RecipientDecision>,
    pub stream: Option<NotificationStreamId>,
}

impl RoutePlan {
    pub fn merge(&mut self, other: Self) {
        if self.stream.is_none() {
            self.stream = other.stream;
        }
        for decision in other.recipients {
            match self.recipients.iter_mut().find(|d| d.user == decision.user) {
                Some(existing) if decision.attention > existing.attention => *existing = decision,
                Some(_) => {},
                None => self.recipients.push(decision),
            }
        }
    }

    #[must_use]
    pub const fn is_empty(&self) -> bool {
        self.recipients.is_empty()
    }
}
