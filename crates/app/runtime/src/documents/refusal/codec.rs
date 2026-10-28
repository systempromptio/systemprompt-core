//! Refusals built from a node's state or a codec failure.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::DocumentNodeId;
use systemprompt_provider_contracts::documents::{ClosestBlock, CodecError};

use super::{Refusal, RefusalCode, Refused};

pub(super) fn guard_mismatch(node: &DocumentNodeId, actual: &str, current_text: &str) -> Refusal {
    Refusal {
        node: Some(node.clone()),
        hash: Some(actual.to_owned()),
        current_text: Some(current_text.to_owned()),
        ..Refusal::new(RefusalCode::GuardMismatch)
    }
}

pub(super) fn find_none(closest: Option<&ClosestBlock>) -> Refusal {
    let mut refusal = Refusal::new(RefusalCode::FindNone);
    refusal.matches = Some(0);
    if let Some(block) = closest {
        refusal.node = Some(block.node.clone());
        refusal.hash = Some(block.hash.clone());
        refusal.current_text = Some(block.text.clone());
    }
    refusal
}

pub(super) fn codec_refusal(error: &CodecError) -> Option<Refused> {
    let message = error.to_string();
    let refusal = match error {
        CodecError::NoMatch => {
            return Some(
                Refusal {
                    matches: Some(0),
                    ..Refusal::new(RefusalCode::FindNone)
                }
                .with_message(
                    "old_str matches 0 places; read the document text and quote old_str exactly \
                     as it appears",
                ),
            );
        },
        CodecError::AmbiguousMatch { matches } => Refusal {
            matches: Some(*matches),
            ..Refusal::new(RefusalCode::FindNone)
        },
        CodecError::UnsupportedOp { .. } => Refusal::new(RefusalCode::UnsupportedOp),
        CodecError::UnknownNode { node } => Refusal {
            node: Some(node.clone()),
            ..Refusal::new(RefusalCode::NotFound)
        },
        CodecError::StaleAnchor {
            node,
            actual,
            current_text,
            ..
        } => guard_mismatch(node, actual, current_text),
        CodecError::Malformed { .. } | CodecError::Parse { .. } => {
            Refusal::new(RefusalCode::InvalidParams)
        },
        CodecError::Refused { .. } => Refusal::new(RefusalCode::UnsupportedOp),
        CodecError::Io(_) => return None,
    };
    Some(refusal.with_message(message))
}
