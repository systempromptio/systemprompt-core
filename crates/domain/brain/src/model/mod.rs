//! The extraction model call.
//!
//! [`request_for`] builds a kind's [`BrainModelRequest`] (versioned prompt,
//! strict output schema, output cap) and [`parse_output`] turns the answer
//! into candidates, refusing any output the schema does not admit.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod parse;
pub mod prompt;
pub mod schema;

pub use parse::parse_output;
pub use prompt::build_prompt;
pub use schema::kind_schema;

use systemprompt_identifiers::FactKindId;
use systemprompt_models::brain::{BrainModelRequest, ExtractorDecl, KindDecl, SourceText};

/// Why a model request could not be built or its output not accepted.
#[derive(Debug, thiserror::Error)]
pub enum ModelOutputError {
    #[error("kind {0} has no model extractor with a prompt version")]
    NoModelExtractor(FactKindId),
    #[error("model output is not JSON")]
    NotJson(#[source] serde_json::Error),
    #[error("model output `span` is malformed")]
    MalformedSpan(#[source] serde_json::Error),
    #[error("model output does not match the schema: {0}")]
    Schema(String),
}

#[must_use]
pub fn request_for(
    kind: &KindDecl,
    text: &SourceText,
    hints: &[String],
    max_output_tokens: u32,
) -> Option<BrainModelRequest> {
    let Some(ExtractorDecl::Model {
        prompt,
        prompt_version: Some(version),
    }) = &kind.extractor
    else {
        return None;
    };
    Some(
        BrainModelRequest::new(build_prompt(kind, prompt, text, hints), kind_schema(kind))
            .with_max_output_tokens(max_output_tokens)
            .with_prompt_version(version.clone()),
    )
}
