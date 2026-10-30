//! The validate stage: core's [`SchemaValidator`] first, then extension
//! validators by ascending priority; the first `Reject` or `Dispute` wins.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashSet;
use std::sync::Arc;

use systemprompt_extension::brain::{FactValidator, ValidationVerdict};
use systemprompt_identifiers::TopicId;
use systemprompt_models::brain::{FactCandidate, KindDecl, SourceText};

use crate::defaults::SchemaValidator;
use crate::error::BrainResult;

/// A validated candidate, or why it was refused.
#[derive(Debug, Clone, PartialEq)]
pub enum Validated {
    Accept(FactCandidate),
    Dispute {
        candidate: FactCandidate,
        reason: String,
    },
    Reject {
        validator: String,
        reason: String,
    },
}

pub async fn validate(
    validators: &[Arc<dyn FactValidator>],
    candidate: FactCandidate,
    kind: &KindDecl,
    text: Option<&SourceText>,
    topics: &HashSet<TopicId>,
) -> BrainResult<Validated> {
    let check = SchemaValidator::check(candidate, kind, text, topics);
    if !check.dropped_topics.is_empty() {
        tracing::debug!(kind = %kind.kind, dropped = check.dropped_topics.len(), "Undeclared topics dropped");
    }
    if let ValidationVerdict::Reject { reason } = check.verdict {
        return Ok(Validated::Reject {
            validator: SchemaValidator::NAME.to_owned(),
            reason,
        });
    }
    let candidate = check.candidate;
    for validator in validators {
        match validator.validate(&candidate, kind, text).await? {
            ValidationVerdict::Accept => {},
            ValidationVerdict::Reject { reason } => {
                return Ok(Validated::Reject {
                    validator: validator.name().to_owned(),
                    reason,
                });
            },
            ValidationVerdict::Dispute { reason } => {
                return Ok(Validated::Dispute { candidate, reason });
            },
        }
    }
    Ok(Validated::Accept(candidate))
}
