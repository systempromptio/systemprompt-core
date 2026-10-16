//! Counting and reporting signed `tool_use` blocks a response could not cache.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_wire::canonical::{CanonicalContent, CanonicalResponse};

use super::{CAPTURE_SKIPPED_TOTAL, ThoughtSignatureCache};

impl ThoughtSignatureCache {
    #[must_use]
    pub fn signed_tool_use_count(response: &CanonicalResponse) -> usize {
        response
            .content
            .iter()
            .filter(|c| {
                matches!(
                    c,
                    CanonicalContent::ToolUse {
                        signature: Some(_),
                        ..
                    }
                )
            })
            .count()
    }

    pub fn note_uncacheable_response(response: &CanonicalResponse, reason: &'static str) {
        let signed = Self::signed_tool_use_count(response);
        if signed == 0 {
            return;
        }
        metrics::counter!(CAPTURE_SKIPPED_TOTAL, "reason" => reason).increment(1);
        tracing::warn!(
            reason,
            signed_tool_use_blocks = signed,
            "thought signatures could not be cached; a later turn in this conversation will miss"
        );
    }
}
