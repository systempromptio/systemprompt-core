//! Extraction inputs and outputs, the source text an extractor reads, and
//! the model request the brain sends through the gateway.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};
use systemprompt_identifiers::{AiRequestId, DocumentNodeId};

use super::candidate::FactCandidate;
use super::envelope::BrainEnvelope;
use super::kind_decl::KindDecl;
use super::source::{FactSource, SourceSpan};

pub const BRAIN_EXTRACT_PURPOSE: &str = "brain.extract";

/// One addressable node of a source's text.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceNode {
    pub node_id: DocumentNodeId,
    pub hash8: String,
    pub text: String,
}

/// The readable text of a source and its nodes, so a cited span can be
/// checked.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct SourceText {
    pub text: String,
    pub nodes: Vec<SourceNode>,
}

impl SourceText {
    #[must_use]
    pub fn has_span(&self, span: &SourceSpan) -> bool {
        self.nodes.iter().any(|node| {
            node.node_id == span.node_id
                && node.hash8 == span.hash8
                && node.text.matches(span.find.as_str()).count() > span.nth as usize
        })
    }
}

/// What one extractor is asked to read for one kind.
#[derive(Debug, Clone, PartialEq, Serialize)]
pub struct ExtractionInput {
    pub envelope: BrainEnvelope,
    pub kind: KindDecl,
    pub source: FactSource,
    pub text: Option<SourceText>,
}

/// The audit record of one extraction call.
#[derive(Debug, Clone, Default, PartialEq, Serialize, Deserialize)]
pub struct ExtractionRecord {
    pub extractor: String,
    pub model: Option<String>,
    pub prompt_version: Option<String>,
    // JSON: the extractor's raw output is kept verbatim for audit and replay.
    pub raw_output: serde_json::Value,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub ai_request_id: Option<AiRequestId>,
}

/// The candidates an extraction produced and its record.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct ExtractionOutput {
    pub candidates: Vec<FactCandidate>,
    pub record: ExtractionRecord,
}

/// The system and user turns of a model call.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
pub struct BrainPrompt {
    pub system: String,
    pub user: String,
}

/// A named strict JSON schema the model's output must satisfy.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct StrictJsonSchema {
    pub name: String,
    // JSON: a JSON Schema document is passed to the provider unchanged.
    pub schema: serde_json::Value,
}

/// A model call the brain makes through the in-process gateway client.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainModelRequest {
    pub system: String,
    pub user: String,
    pub json_schema: StrictJsonSchema,
    pub max_output_tokens: u32,
    pub prompt_version: String,
    pub purpose: String,
}

impl BrainModelRequest {
    pub const DEFAULT_MAX_OUTPUT_TOKENS: u32 = 2048;

    #[must_use]
    pub fn new(prompt: BrainPrompt, json_schema: StrictJsonSchema) -> Self {
        Self {
            system: prompt.system,
            user: prompt.user,
            json_schema,
            max_output_tokens: Self::DEFAULT_MAX_OUTPUT_TOKENS,
            prompt_version: String::new(),
            purpose: BRAIN_EXTRACT_PURPOSE.to_owned(),
        }
    }

    #[must_use]
    pub const fn with_max_output_tokens(mut self, max_output_tokens: u32) -> Self {
        self.max_output_tokens = max_output_tokens;
        self
    }

    #[must_use]
    pub fn with_prompt_version(mut self, prompt_version: impl Into<String>) -> Self {
        self.prompt_version = prompt_version.into();
        self
    }

    #[must_use]
    pub fn with_purpose(mut self, purpose: impl Into<String>) -> Self {
        self.purpose = purpose.into();
        self
    }
}

/// The model's answer and its metering.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct BrainModelResponse {
    pub model: String,
    pub text: String,
    // JSON: structured output parsed against the request's strict schema.
    pub structured: Option<serde_json::Value>,
    pub input_tokens: u32,
    pub output_tokens: u32,
    pub ai_request_id: Option<AiRequestId>,
}
