//! Brain identifiers.
//!
//! [`BrainFactId`] is `m_<uuid>`: the prefix is user-visible in recalled
//! memory blocks and response headers, so it is minted by a hand-written
//! `generate()` and checked on parse. Kinds, topics, interests and workflows
//! are declared keys from `services/brain/**`, checked to 1-64 characters of
//! lowercase ASCII letters, digits, `_`, `.` and `-`. Extractions,
//! proposals, recalls, disputes and fact revisions are UUID-backed.
//! [`PipelineVersion`] is the 16 lowercase hex characters of the pipeline
//! digest.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::error::IdValidationError;

crate::define_id!(BrainFactId, checked, validate_fact_id);
crate::define_id!(FactKindId, checked, |value| validate_key(
    "FactKindId",
    value
));
crate::define_id!(TopicId, checked, |value| validate_key("TopicId", value));
crate::define_id!(InterestId, checked, |value| validate_key(
    "InterestId",
    value
));
crate::define_id!(WorkflowId, checked, |value| validate_key(
    "WorkflowId",
    value
));
crate::define_id!(BrainExtractionId, uuid);
crate::define_id!(BrainProposalId, uuid);
crate::define_id!(BrainRecallId, uuid);
crate::define_id!(BrainDisputeId, uuid);
crate::define_id!(BrainFactRevisionId, uuid);
crate::define_id!(PipelineVersion, checked, validate_pipeline_version);

const FACT_ID_PREFIX: &str = "m_";
const KEY_MAX_LEN: usize = 64;
const PIPELINE_VERSION_LEN: usize = 16;

impl BrainFactId {
    pub fn generate() -> Self {
        Self(format!("{FACT_ID_PREFIX}{}", uuid::Uuid::new_v4()))
    }
}

fn validate_fact_id(value: &str) -> Result<(), IdValidationError> {
    let Some(rest) = value.strip_prefix(FACT_ID_PREFIX) else {
        return Err(IdValidationError::invalid(
            "BrainFactId",
            "must start with 'm_'",
        ));
    };
    crate::macros::validate_uuid("BrainFactId", rest)
}

fn validate_key(name: &'static str, value: &str) -> Result<(), IdValidationError> {
    if value.is_empty() {
        return Err(IdValidationError::empty(name));
    }
    if value.len() > KEY_MAX_LEN {
        return Err(IdValidationError::invalid(
            name,
            "must be at most 64 characters",
        ));
    }
    if !value.bytes().all(|b| {
        b.is_ascii_lowercase() || b.is_ascii_digit() || b == b'_' || b == b'.' || b == b'-'
    }) {
        return Err(IdValidationError::invalid(
            name,
            "must contain only lowercase letters, digits, '_', '.' and '-'",
        ));
    }
    Ok(())
}

fn validate_pipeline_version(value: &str) -> Result<(), IdValidationError> {
    if value.len() != PIPELINE_VERSION_LEN
        || !value
            .bytes()
            .all(|b| b.is_ascii_digit() || (b'a'..=b'f').contains(&b))
    {
        return Err(IdValidationError::invalid(
            "PipelineVersion",
            "must be 16 lowercase hex characters",
        ));
    }
    Ok(())
}
