//! Normalised invocation facts: the fact key, consumer and resource
//! attribution, and the spend a consumer recorded for one skill invocation.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use chrono::{DateTime, Utc};
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{
    AnalyticsChangeId, AnalyticsFactId, DeviceId, ManagedResourceId, MarketplaceId,
    NativeSessionId, PluginId, ResourceInvocationId, ResourceRevisionId, UserId,
};

use super::EvaluatorClient;

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(rename_all = "snake_case")]
pub enum AnalyticsFactKind {
    Invocation,
    Request,
    Assessment,
    ResourceAssociation,
    Artifact,
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

/// The skill an invocation named, as the hook reported it, and the services
/// source that shipped it.
///
/// Distinct from [`InvocationResourceAttribution`]: that is the revision
/// proof (a managed resource this device verifiably installed), this is the
/// identity every invocation has whether or not the skill is a managed
/// resource. `skill` is the hook's `<plugin>:<skill>` string verbatim;
/// `source` is `base` or `bundle:<name>` and `source_hash` is that source's
/// content hash at the time the fact was normalised, so a figure keyed on
/// the skill can also say which published tree it ran from.
/// `marketplace_hash` is the content hash of the marketplace version the
/// plugin was served from, so a figure can be pinned to one published
/// marketplace even after the source moves on.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct InvocationSkillIdentity {
    pub plugin_id: PluginId,
    pub skill: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketplace_id: Option<MarketplaceId>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub source_hash: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub marketplace_hash: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NormalizedInvocationFact {
    pub invocation_id: ResourceInvocationId,
    pub occurred_at: DateTime<Utc>,
    pub consumer: InvocationConsumerIdentity,
    pub attribution: InvocationResourceAttribution,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<InvocationSkillIdentity>,
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

/// One tool result stored as a typed artifact.
///
/// It is joined to the invocation and request it belongs to where those are
/// known. `source` is the vantage point the platform saw the result from and
/// `correlation` says whether it was joined by an exact key or inferred.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct NormalizedArtifactFact {
    pub artifact_key: AnalyticsFactKey,
    pub execution_id: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub invocation_key: Option<AnalyticsFactKey>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_key: Option<AnalyticsFactKey>,
    pub occurred_at: DateTime<Utc>,
    pub consumer: InvocationConsumerIdentity,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub skill: Option<InvocationSkillIdentity>,
    pub tool_name: String,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub server_name: Option<String>,
    pub artifact_type: String,
    pub source: crate::mcp::ExecutionSource,
    pub correlation: crate::mcp::Correlation,
    pub is_structured: bool,
    pub has_ui_resource: bool,
    pub succeeded: bool,
    pub payload_bytes: Option<u64>,
    pub findings: u64,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(tag = "kind", content = "value", rename_all = "snake_case")]
pub enum NormalizedAnalyticsFact {
    Invocation(NormalizedInvocationFact),
    Request(NormalizedRequestFact),
    Assessment(NormalizedAssessmentFact),
    ResourceAssociation(NormalizedResourceAssociationFact),
    Artifact(Box<NormalizedArtifactFact>),
}

impl NormalizedAnalyticsFact {
    pub const fn kind(&self) -> AnalyticsFactKind {
        match self {
            Self::Invocation(_) => AnalyticsFactKind::Invocation,
            Self::Request(_) => AnalyticsFactKind::Request,
            Self::Assessment(_) => AnalyticsFactKind::Assessment,
            Self::ResourceAssociation(_) => AnalyticsFactKind::ResourceAssociation,
            Self::Artifact(_) => AnalyticsFactKind::Artifact,
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
