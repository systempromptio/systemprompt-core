//! Normalised invocation facts: the fact key, consumer and resource
//! attribution, and the spend a consumer recorded for one skill invocation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    AnalyticsChangeId, AnalyticsFactId, DeviceId, ManagedResourceId, NativeSessionId,
    ResourceInvocationId, ResourceRevisionId, UserId,
};

use super::EvaluatorClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsFactKind {
    Invocation,
    Request,
    Assessment,
    ResourceAssociation,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalyticsFactKey {
    pub kind: AnalyticsFactKind,
    pub source: String,
    pub id: AnalyticsFactId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InvocationConsumerIdentity {
    HistoricalUnknown,
    Authenticated {
        consumer_id: UserId,
        device_id: DeviceId,
        host: EvaluatorClient,
        session_id: NativeSessionId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum InvocationResourceAttribution {
    Unknown,
    Verified {
        resource_id: ManagedResourceId,
        revision_id: ResourceRevisionId,
    },
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NormalizedInvocationFact {
    pub invocation_id: ResourceInvocationId,
    pub occurred_at: DateTime<Utc>,
    pub consumer: InvocationConsumerIdentity,
    pub attribution: InvocationResourceAttribution,
    pub succeeded: bool,
    pub latency_micros: Option<u64>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum RecordedSpend {
    Known {
        currency: String,
        amount_micros: u64,
    },
    UnknownPricing,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NormalizedRequestFact {
    pub request_key: AnalyticsFactKey,
    pub occurred_at: DateTime<Utc>,
    pub consumer: InvocationConsumerIdentity,
    pub succeeded: bool,
    pub spend: RecordedSpend,
    pub input_tokens: Option<u64>,
    pub output_tokens: Option<u64>,
    pub latency_micros: Option<u64>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "status", rename_all = "snake_case")]
pub enum AssessmentOutcome {
    Scored { score_millionths: i64 },
    Failed,
    Unavailable,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AssessmentConversationKey {
    pub source: String,
    pub id: AnalyticsFactId,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NormalizedAssessmentFact {
    pub conversation_key: AssessmentConversationKey,
    pub assessment_key: AnalyticsFactKey,
    pub invocation_key: AnalyticsFactKey,
    pub occurred_at: DateTime<Utc>,
    pub outcome: AssessmentOutcome,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NormalizedResourceAssociationFact {
    pub association_key: AnalyticsFactKey,
    pub invocation_key: AnalyticsFactKey,
    pub request_key: AnalyticsFactKey,
    pub occurred_at: DateTime<Utc>,
    pub attribution: InvocationResourceAttribution,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum NormalizedAnalyticsFact {
    Invocation(NormalizedInvocationFact),
    Request(NormalizedRequestFact),
    Assessment(NormalizedAssessmentFact),
    ResourceAssociation(NormalizedResourceAssociationFact),
}

impl NormalizedAnalyticsFact {
    pub const fn kind(&self) -> AnalyticsFactKind {
        match self {
            Self::Invocation(_) => AnalyticsFactKind::Invocation,
            Self::Request(_) => AnalyticsFactKind::Request,
            Self::Assessment(_) => AnalyticsFactKind::Assessment,
            Self::ResourceAssociation(_) => AnalyticsFactKind::ResourceAssociation,
        }
    }
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "operation", rename_all = "snake_case")]
#[expect(
    clippy::large_enum_variant,
    reason = "wire contract: a tombstone carries nothing by definition and the fact is matched by value"
)]
pub enum AnalyticsChangeOperation {
    Replace { fact: NormalizedAnalyticsFact },
    Tombstone,
}

#[derive(Debug, Clone, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct AnalyticsChange {
    pub change_id: AnalyticsChangeId,
    pub key: AnalyticsFactKey,
    pub revision: u64,
    pub occurred_at: DateTime<Utc>,
    pub recorded_at: DateTime<Utc>,
    pub operation: AnalyticsChangeOperation,
}

impl AnalyticsChange {
    pub fn validate(&self) -> Result<(), super::FeedbackContractError> {
        if self.revision == 0 || self.key.source.is_empty() || self.key.source.len() > 128 {
            return Err(super::FeedbackContractError::Bounds);
        }
        if let AnalyticsChangeOperation::Replace { fact } = &self.operation
            && fact.kind() != self.key.kind
        {
            return Err(super::FeedbackContractError::IncompleteManifest);
        }
        Ok(())
    }
}
