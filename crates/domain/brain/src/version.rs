//! The pipeline version a fact is built under: the declared plane hash, the
//! brain code version and every model prompt version, digested by
//! [`PipelineVersionInputs::derive`].
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use systemprompt_identifiers::{FactKindId, PipelineVersion};
pub use systemprompt_models::brain::BRAIN_CODE_VERSION;
use systemprompt_models::brain::PipelineVersionInputs;

#[must_use]
pub fn pipeline_inputs(
    declared_hash: impl Into<String>,
    prompt_versions: BTreeMap<FactKindId, String>,
) -> PipelineVersionInputs {
    prompt_versions.into_iter().fold(
        PipelineVersionInputs::new(declared_hash),
        |inputs, (kind, version)| inputs.with_prompt_version(kind, version),
    )
}

#[must_use]
pub fn pipeline_version(
    declared_hash: impl Into<String>,
    prompt_versions: BTreeMap<FactKindId, String>,
) -> PipelineVersion {
    pipeline_inputs(declared_hash, prompt_versions).derive()
}
