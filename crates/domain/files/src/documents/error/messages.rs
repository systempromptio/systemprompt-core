//! The longer [`DocumentsError`](super::DocumentsError) messages, each
//! naming what went wrong and what to do next.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::{
    ContentHash, DocumentFolderId, DocumentId, DocumentNodeId, DocumentOpId,
};
use systemprompt_models::documents::{AccessLevel, DocumentFormat};

pub(super) fn stale_anchor(
    node: &DocumentNodeId,
    expected: &str,
    actual: &str,
    current_text: &str,
) -> String {
    format!(
        "node {node} changed since it was read (expected hash {expected}, now {actual}); its \
         current text is \"{current_text}\". The head edit wins: re-aim your change at this \
         text with hash {actual}, and never resend your old text over it"
    )
}

pub(super) fn unknown_node(node: &DocumentNodeId) -> String {
    format!(
        "node {node} does not exist at the head revision (deleted, or not an id of this \
         document); read the outline again for current node ids"
    )
}

pub(super) fn trashed(document_id: &DocumentId) -> String {
    format!(
        "document {document_id} is in the trash; restore it with documents_untrash \
         before reading, editing or sharing it"
    )
}

pub(super) fn op_id_mismatch(op_id: &DocumentOpId) -> String {
    format!(
        "op id {op_id} was already used for a different request; send a new op id for a new \
         write"
    )
}

pub(super) fn guard_missing(op: &str, node: &DocumentNodeId) -> String {
    format!(
        "{op} on node {node} needs expected_hash, the node's hash8 from your last read; a quote \
         target resolves its own"
    )
}

pub(super) fn integrity_mismatch(declared: &ContentHash, actual: &ContentHash) -> String {
    format!(
        "uploaded bytes hash to {actual} but the commit declared {declared}; the upload was \
         corrupted, send it again"
    )
}

pub(super) fn invalid_name(name: &str) -> String {
    format!(
        "\"{name}\" is not a valid name: use 1-255 characters without '/', '\\', control \
         characters, \".\" or \"..\""
    )
}

pub(super) fn name_taken(name: &str) -> String {
    format!(
        "a document named \"{name}\" already exists in this folder; choose another name or \
         edit that document"
    )
}

pub(super) fn unknown_subject(kind: &str, value: &str) -> String {
    format!(
        "no active {kind} \"{value}\" exists; name an existing, active subject (a user id for \
         a user)"
    )
}

pub(super) fn share_below_existing(existing: &AccessLevel, held: &AccessLevel) -> String {
    format!(
        "forbidden: the subject already holds {existing} here, above your {held}; only a holder \
         of {existing} or the owner can change that grant"
    )
}

pub(super) fn subtree_changed(folder_id: &DocumentFolderId) -> String {
    format!(
        "folder {folder_id} changed while it was being deleted or moved (a document or subfolder \
         was added, moved or given away, or the destination moved); nothing changed, try again"
    )
}

pub(super) fn ownership_changed(document_id: &DocumentId) -> String {
    format!(
        "document {document_id} changed owner while the transfer was in flight; nothing was \
         transferred"
    )
}

pub(super) fn access_request_open(document_id: &DocumentId) -> String {
    format!(
        "you already have an open access request on document {document_id}; wait for the \
         owner to answer it"
    )
}

pub(super) fn access_request_not_needed(document_id: &DocumentId, held: &AccessLevel) -> String {
    format!(
        "you already hold {held} access to document {document_id}; ask someone who holds share \
         to raise it"
    )
}

pub(super) fn live_unsupported(format: &DocumentFormat) -> String {
    format!(
        "live editing is not available for {format} documents; edit them with document ops \
         (documents_edit) instead"
    )
}
