//! The `files.documents.limits` section: per-request input limits of the
//! document store, the PDF export page cap and the ceilings on agent edits
//! per hour (per agent of a user, and for all of a user's agents), each with a
//! floor and a ceiling an operator may set between.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

const KIB: u64 = 1024;
const MIB: u64 = 1024 * KIB;

const OPS_PER_CALL_BOUNDS: (u64, u64) = (1, 10_000);
const OP_TEXT_BOUNDS: (u64, u64) = (KIB, 64 * MIB);
const MARKDOWN_BOUNDS: (u64, u64) = (KIB, 64 * MIB);
const COMMENT_BOUNDS: (u64, u64) = (256, MIB);
const COLLAB_UPDATE_BOUNDS: (u64, u64) = (KIB, 64 * MIB);
const CURSOR_BOUNDS: (u64, u64) = (256, 64 * KIB);
const PDF_PAGES_BOUNDS: (u64, u64) = (1, 5_000);
const AGENT_EDITS_BOUNDS: (u64, u64) = (1, 1_000_000);

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase", default)]
pub struct DocumentLimits {
    pub max_ops_per_call: u64,
    pub max_op_text_bytes: u64,
    pub max_markdown_bytes: u64,
    pub max_comment_bytes: u64,
    pub max_collab_update_bytes: u64,
    pub max_cursor_bytes: u64,
    pub pdf_pages: u64,
    pub agent_edits_per_hour: u64,
    pub agent_edits_per_hour_per_user: u64,
}

impl Default for DocumentLimits {
    fn default() -> Self {
        Self {
            max_ops_per_call: 200,
            max_op_text_bytes: MIB,
            max_markdown_bytes: MIB,
            max_comment_bytes: 16 * KIB,
            max_collab_update_bytes: 4 * MIB,
            max_cursor_bytes: 4 * KIB,
            pdf_pages: 500,
            agent_edits_per_hour: 600,
            agent_edits_per_hour_per_user: 1_500,
        }
    }
}

impl DocumentLimits {
    #[must_use]
    pub const fn bounded(&self) -> [(&'static str, u64, (u64, u64)); 9] {
        [
            ("maxOpsPerCall", self.max_ops_per_call, OPS_PER_CALL_BOUNDS),
            ("maxOpTextBytes", self.max_op_text_bytes, OP_TEXT_BOUNDS),
            ("maxMarkdownBytes", self.max_markdown_bytes, MARKDOWN_BOUNDS),
            ("maxCommentBytes", self.max_comment_bytes, COMMENT_BOUNDS),
            (
                "maxCollabUpdateBytes",
                self.max_collab_update_bytes,
                COLLAB_UPDATE_BOUNDS,
            ),
            ("maxCursorBytes", self.max_cursor_bytes, CURSOR_BOUNDS),
            ("pdfPages", self.pdf_pages, PDF_PAGES_BOUNDS),
            (
                "agentEditsPerHour",
                self.agent_edits_per_hour,
                AGENT_EDITS_BOUNDS,
            ),
            (
                "agentEditsPerHourPerUser",
                self.agent_edits_per_hour_per_user,
                AGENT_EDITS_BOUNDS,
            ),
        ]
    }
}
