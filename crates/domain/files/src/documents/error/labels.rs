//! The prose fragments [`DocumentsError`](super::DocumentsError) messages are
//! assembled from.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{DocumentCommentId, DocumentNodeId, DocumentRevisionId, UserId};
use systemprompt_models::documents::AccessLevel;
use systemprompt_provider_contracts::documents::{ClosestBlock, DocumentFormat};

pub(super) fn base_label(base: Option<&DocumentRevisionId>) -> String {
    base.map_or_else(
        || "an unknown base revision".to_owned(),
        |base| format!("base revision {base}"),
    )
}

pub(super) fn kinds_label(op: &str, format: &DocumentFormat, kinds: &[&str]) -> String {
    if format.is_file() {
        return "; a file document (a PDF, an image, any other upload) is stored bytes and is never \
                edited: download or export it, or write a new version of the whole file"
            .to_owned();
    }
    match (op, kinds) {
        ("insert_after", []) => "; this format takes no insert_after".to_owned(),
        ("insert_after", kinds) => format!("; insert_after accepts kind {}", kinds.join(", ")),
        _ => String::new(),
    }
}

pub(super) fn closest_label(closest: Option<&ClosestBlock>) -> String {
    closest.map_or_else(String::new, |block| {
        format!(
            "; the closest block is {} (hash {}): \"{}\"",
            block.node, block.hash, block.text
        )
    })
}

pub(super) fn held_label(held: Option<&AccessLevel>) -> String {
    held.map_or_else(|| "no access".to_owned(), |level| format!("{level} access"))
}

pub(super) fn policy_label(policy: Option<&str>, owner: Option<&UserId>) -> String {
    match (policy, owner) {
        (Some(policy), _) => format!(" but policy {policy} denied the request"),
        (None, Some(owner)) => {
            format!(". Ask the owner, user {owner}, to share it with you at that level")
        },
        (None, None) => ". Ask the owner to share it with you at that level".to_owned(),
    }
}

pub(super) fn comment_list(comments: &[DocumentCommentId]) -> String {
    comments
        .iter()
        .map(DocumentCommentId::as_str)
        .collect::<Vec<_>>()
        .join(", ")
}

pub(super) fn conflict_label(node: Option<&DocumentNodeId>, current_text: Option<&str>) -> String {
    match (node, current_text) {
        (Some(node), Some(text)) => {
            format!(": node {node} changed since the proposal; its current text is \"{text}\"")
        },
        (Some(node), None) => format!(": node {node} no longer exists"),
        _ => String::new(),
    }
}
