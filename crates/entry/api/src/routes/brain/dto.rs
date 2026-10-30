//! The request and response bodies of the brain routes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_brain::service::DisputeOutcome;
use systemprompt_extension::brain::{InjectedFact, RecallBlock, RecallOutcome, WithheldReason};
use systemprompt_identifiers::{BrainDisputeId, BrainFactId, EventOutboxId, UserId};
use systemprompt_models::origin::ClientKind;

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SearchParams {
    #[serde(default)]
    pub kind: Option<String>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub status: Option<String>,
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct WhyParams {
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub client: Option<ClientKind>,
}

#[derive(Debug, Clone, Copy, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct LimitParams {
    #[serde(default)]
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EnterAccepted {
    pub outbox_id: EventOutboxId,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct DisputeBody {
    pub reason: String,
}

#[derive(Debug, Serialize)]
pub struct DisputeCreated {
    pub dispute_id: BrainDisputeId,
}

#[derive(Debug, Default, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct RetractBody {
    #[serde(default)]
    pub reason: String,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct FieldsBody {
    // JSON: edited fields are checked against the kind's template by the service.
    pub fields: serde_json::Map<String, serde_json::Value>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum ResolveOutcome {
    Upheld,
    Dismissed,
}

impl From<ResolveOutcome> for DisputeOutcome {
    fn from(outcome: ResolveOutcome) -> Self {
        match outcome {
            ResolveOutcome::Upheld => Self::Upheld,
            ResolveOutcome::Dismissed => Self::Dismissed,
        }
    }
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct ResolveBody {
    pub fact_id: BrainFactId,
    pub outcome: ResolveOutcome,
    #[serde(default)]
    pub note: Option<String>,
}

#[derive(Debug, Deserialize)]
#[serde(deny_unknown_fields)]
pub struct SimulateBody {
    pub user_id: UserId,
    pub client: ClientKind,
    #[serde(default)]
    pub project: Option<String>,
    #[serde(default)]
    pub prompt: String,
    #[serde(default)]
    pub reuse: bool,
}

#[derive(Debug, Serialize)]
pub struct SimulatedFact {
    pub id: BrainFactId,
    pub revision: i32,
    pub source: String,
    pub text: String,
}

impl From<InjectedFact> for SimulatedFact {
    fn from(fact: InjectedFact) -> Self {
        Self {
            id: fact.id,
            revision: fact.revision,
            source: fact.source,
            text: fact.text,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SimulatedBlock {
    pub scope: String,
    pub as_of: DateTime<Utc>,
    pub facts: Vec<SimulatedFact>,
    pub withheld: BTreeMap<WithheldReason, u32>,
    pub tokens: u32,
    pub rendered: String,
}

impl From<RecallBlock> for SimulatedBlock {
    fn from(block: RecallBlock) -> Self {
        let rendered = block.render();
        Self {
            scope: block.scope,
            as_of: block.as_of,
            facts: block.facts.into_iter().map(SimulatedFact::from).collect(),
            withheld: block.withheld.by_reason,
            tokens: block.tokens,
            rendered,
        }
    }
}

#[derive(Debug, Serialize)]
pub struct SimulateResponse {
    pub descriptor: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<&'static str>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block: Option<SimulatedBlock>,
}

impl From<RecallOutcome> for SimulateResponse {
    fn from(outcome: RecallOutcome) -> Self {
        let descriptor = outcome.descriptor();
        match outcome {
            RecallOutcome::Injected(block) => Self {
                descriptor,
                skipped: None,
                block: Some(SimulatedBlock::from(block)),
            },
            RecallOutcome::Skipped { reason } => Self {
                descriptor,
                skipped: Some(reason.as_str()),
                block: None,
            },
        }
    }
}
