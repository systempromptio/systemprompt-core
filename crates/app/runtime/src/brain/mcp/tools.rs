//! The brain tools' inputs, outputs and `tools/list` shape.
//!
//! Every input refuses an unknown key. Outputs are terse and reach the wire
//! once, as structured content. A listed tool keeps its input schema with
//! the schemars title and `$schema` stripped and advertises no output
//! schema, so the list stays small; the working detail is in `brain_guide`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;
use std::sync::Arc;

use rmcp::model::Tool;
use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{BrainDisputeId, BrainFactId, EventOutboxId, FactKindId, TopicId};
use systemprompt_mcp::McpOutputSchema;
use systemprompt_models::brain::{BrainFact, FactScope, FactSource, FactStatus, ScopeLevel};

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct SearchInput {
    pub query: String,
    #[serde(default)]
    pub kind: Option<FactKindId>,
    #[serde(default)]
    pub subject: Option<String>,
    #[serde(default)]
    pub scope: Option<ScopeLevel>,
    #[serde(default)]
    pub limit: Option<i64>,
}

#[derive(Debug, Clone, PartialEq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ProposeInput {
    pub kind: FactKindId,
    pub subject: String,
    // JSON: protocol boundary; fields are checked against the kind declaration by the pipeline.
    pub fields: serde_json::Map<String, serde_json::Value>,
    #[serde(default)]
    pub topics: Option<Vec<TopicId>>,
    #[serde(default)]
    pub scope: Option<FactScope>,
    #[serde(default)]
    pub status: Option<FactStatus>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct DisputeInput {
    pub fact_id: BrainFactId,
    pub reason: String,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct WhyInput {
    pub fact_id: BrainFactId,
    #[serde(default)]
    pub project: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct RecallPreviewInput {
    pub prompt: String,
    #[serde(default)]
    pub project: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize, JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct GuideInput {
    #[serde(default)]
    pub topics: Vec<String>,
}

macro_rules! brain_output {
    ($ty:ty, $name:literal) => {
        impl McpOutputSchema for $ty {
            fn artifact_type() -> &'static str {
                $name
            }

            fn terse_wire(&self) -> bool {
                true
            }
        }
    };
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct FactItem {
    pub id: BrainFactId,
    pub revision: i32,
    pub kind: FactKindId,
    pub subject: String,
    pub statement: String,
    pub status: FactStatus,
    pub source: FactSource,
}

impl From<BrainFact> for FactItem {
    fn from(fact: BrainFact) -> Self {
        Self {
            id: fact.id,
            revision: fact.revision,
            kind: fact.kind,
            subject: fact.subject.to_string(),
            statement: fact.statement,
            status: fact.status,
            source: fact.source,
        }
    }
}

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct SearchOutput {
    pub facts: Vec<FactItem>,
}
brain_output!(SearchOutput, "brain_search");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct ProposeOutput {
    pub outbox_id: EventOutboxId,
}
brain_output!(ProposeOutput, "brain_propose");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct DisputeOutput {
    pub dispute_id: BrainDisputeId,
}
brain_output!(DisputeOutput, "brain_dispute");

#[derive(Debug, Clone, PartialEq, Serialize, JsonSchema)]
pub struct WhyOutput {
    pub fact_id: BrainFactId,
    pub revision: i32,
    pub base: f64,
    pub subject: f64,
    pub workflow: f64,
    pub recency: f64,
    pub total: f64,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub withheld: Option<String>,
}
brain_output!(WhyOutput, "brain_why");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct RecallPreviewOutput {
    pub injected: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub skipped: Option<String>,
    pub withheld: BTreeMap<String, u32>,
    pub tokens: u32,
}
brain_output!(RecallPreviewOutput, "brain_recall_preview");

#[derive(Debug, Clone, PartialEq, Eq, Serialize, JsonSchema)]
pub struct GuideOutput {
    pub topics: Vec<String>,
    pub guide: String,
}
brain_output!(GuideOutput, "brain_guide");

#[must_use]
pub(super) fn compact(mut tool: Tool) -> Tool {
    let mut schema = (*tool.input_schema).clone();
    schema.remove("$schema");
    schema.remove("title");
    tool.input_schema = Arc::new(schema);
    tool.output_schema = None;
    tool.meta = None;
    tool
}
