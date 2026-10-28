//! The documents MCP server's tool list and the instructions it gives a
//! client on connect.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub const TOOL_NAMES: [&str; 18] = [
    "documents_list",
    "documents_search",
    "documents_create",
    "documents_read",
    "documents_edit",
    "documents_comment",
    "documents_history",
    "documents_proposals",
    "documents_share",
    "documents_access",
    "documents_copy",
    "documents_update",
    "documents_trash",
    "documents_untrash",
    "documents_export",
    "documents_upload",
    "documents_guide",
    "documents_people",
];

pub const INSTRUCTIONS: &str = "Shared documents (Word, Excel, PowerPoint, OpenDocument, HTML, \
Markdown, text) you may view, edit and share.
1. Read the outline first: documents_read view outline; lines are `<node>:<hash>| <text>` and \
node ids are stable across revisions.
2. Edit by node id and hash, or by quote, in one documents_edit call; only a touched node that \
changed refuses. Pass op_id so a retry is safe.
3. Anchor the next edit on the ack; resume later with view changes since your last revision.
4. On guard_mismatch or stale_base the head edit wins: re-aim from the returned current text, \
never resend your old text.
5. Do not re-read between your own writes. Details: documents_guide.
6. An edit may land as a proposal (proposal_id in the ack); a reviewer accepts it with \
documents_proposals.
7. To bring a file in, documents_upload it and import the returned blob_ref.";
