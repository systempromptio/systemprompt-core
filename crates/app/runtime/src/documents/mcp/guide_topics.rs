//! The text of the `documents_guide` topics and refusal explanations.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::refusal::RefusalCode;

pub(super) const EDITING: &str = "\
Read: documents_read view outline first; each line is `<node>:<hash>| <text>`, with `# ` per \
heading level, `- ` for a list item, `| ` for a table cell and `## slide N` at each slide. The \
prefix is display only. Node ids are stable: an id names the same block in every revision \
until the block is deleted. Long lists fold to `gap blocks=N`; view node {node_id} pages one \
node and its children; view search {text} finds blocks; view meta gives head, size and who is \
in the document now; view changes {since_revision} returns only the blocks changed since that \
revision, `gap blocks=N` for unchanged runs and `gone <id>` for deletions; a block that \
only moved is not reported as changed.
Edit: documents_edit {document_id, base_revision?, ops, message, op_id?}. Ops: replace_text \
{node, expected_hash, text}; delete {node, expected_hash}; insert_after {node, kind: {type: \
heading, level} | {type: paragraph} | {type: list_item} | {type: row} | {type: text}, text}; \
str_replace {old_str, new_str} (old_str must occur once); set_cells {sheet, range, values}; \
insert_slide {after, layout, title, body}; set_text {text}; set_html {html}; append_markdown \
{markdown}; insert_from_revision {revision_id, node, after} copies a block back from history; \
restore {revision_id} alone makes an old revision the head (base_revision must be the head).
Rich ops (html, markdown, docx, odt; a batch of only these): set_marks {node, expected_hash, \
find?, nth?, add: [{mark: bold|italic|underline|strike|code|link|color|highlight, value?}], \
remove: [mark]}; set_block_type {node, expected_hash, block: {type: paragraph} | {type: \
heading, level} | {type: list_item, level?, ordered?} | {type: quote} | {type: code}}; \
insert_table {after, rows: [[text]]}; table_insert_row / table_insert_col {table, \
expected_hash, at?, cells?}; table_delete {table, expected_hash, row? | col?} (neither deletes \
the table); insert_image {after, blob, alt?} (html and markdown only). Rows and columns count \
from 0. docx and odt take no quote or code blocks, odt takes no marks.
Target: an anchored op names node by id with expected_hash (required), or node: {find: \"quoted text\", \
nth?, within?: node} to hit the block containing the quote; no match refuses find_none with \
the closest block. Content: replace_text and insert_after take as: \"markdown\" to write \
headings, lists, tables, bold, italic and links as native formatting.
Guards: ops apply to the head. An edit is refused only when a node it touches changed since \
you read it; edits elsewhere in the document land and the ack says merged.
Batch: every target resolves against the document as it stood before the call; ops apply in \
order, so two insert_after on one node land in op order; all ops apply or none.
Ack: an edit or create returns `written`, the lines it changed or created; anchor the next \
edit on them. Do not re-read between your own writes. op_id: a retry with the same op_id and \
the same request returns the first ack with replayed instead of applying twice, over MCP or \
REST alike; reusing an op_id for a different request refuses op_id_mismatch.
Limits: at most 200 ops per call and 1 MiB of text per op or of markdown, unless the operator \
set other limits; larger calls refuse too_large.
Skeleton then fill: lay a document out first with insert_after kind {type: pending} (or ops \
in documents_create), whose text is the intent of the section, e.g. \"Goals: three outcomes\". \
A pending block reads as kind pending with its intent as text; replace_text on its id lands the \
real content in its place. In append_markdown a line `<!-- pending: intent -->` is a pending \
block.
Comments: an edit that removes a node or quote a comment is anchored to refuses \
anchors_affected; pass allow_detach: true to land it anyway. answering: <comment_id> records \
the edit's revision on that comment.
Conflict: a guard_mismatch or stale_base means someone else edited first. The head edit wins: \
re-aim from the returned current text and never resend your old text over it.
Terse output: verbose: true on read, edit, history or list adds URIs, content hashes, codec \
paths and revision attribution.";

pub(super) const SHEETS: &str = "\
Cells read as `<sheet>!<cell>:<hash>| <value>` with view cells, sheet and an A1 range such as \
A1:C9. set_cells {sheet, range, values} writes a rectangle: values is rows of cells, each a \
number, bool, string or null, and its shape must match the range; a string starting with = is \
a formula. Rows and cells are named by the node ids a read returns: insert_after on a row \
adds a row below it from tab-separated text; replace_text rewrites a row or a cell; delete \
clears it.";

pub(super) const SLIDES: &str = "\
Slides and their paragraphs are named by the node ids a read returns (verbose shows each \
id's position, slide[N]/shape[H]/p[P]). insert_slide {after: <slide id>, layout, title, body: \
[lines]} adds a slide; delete on a slide id removes it. insert_after on a paragraph takes kind \
paragraph or list_item. Every id in one call names the deck as read.";

pub(super) const SHARING: &str = "\
documents_share {document_id | folder_id, subject_kind, subject_value, level, valid_until}. \
subject_kind is user, role or a registered subject dimension such as group or project; level \
is view, comment, edit or share (own stays with the owner). A folder grant covers its subtree. \
Sharing needs share access, and you cannot change a grant above your own level. \
documents_share {document_id, transfer_to: user_id} makes that active user the owner and keeps \
edit for the previous owner; {document_id, revoke_all: true} removes every grant on the \
document; only the owner can do either. documents_access {document_id} reads back the owner \
and your level and, with share, each grant (expired: true once past valid_until) and the \
grants inherited from folders.";

pub(super) const COMMENTS: &str = "\
documents_comment {action: create, document_id, node, anchor?: {find, nth?}, body} anchors a \
comment to a node id from a read and, with anchor, to the nth (0-based) occurrence of find \
inside it. A reply is {action: create, document_id, parent_id, body}. {action: list, \
document_id} returns every thread: id, anchor, author, body, parent_id, resolved_at and \
answering_revision_id. {action: resolve, document_id, comment_id, answering?: revision_id} \
resolves it. Anchors are stable ids, so edits elsewhere never move them; an edit that deletes \
the node or the quoted words refuses anchors_affected unless allow_detach: true, and a \
detached comment keeps its node and drops its quote. documents_edit answering: <comment_id> \
records the edit's revision on the comment. mentions: [user_id] on create or edit notifies \
users who hold access. {action: edit, document_id, comment_id, body, mentions?} rewrites your \
own comment; {action: delete, document_id, comment_id} deletes yours (or any, with share \
access) and returns the remaining threads.";

pub(super) const LIFECYCLE: &str = "\
documents_trash {document_id} moves a document to the trash: it leaves listings, and reads, \
edits and shares refuse trashed. documents_untrash \
{document_id} brings it back; trashed documents are deleted for good after the retention \
period. Both need own. documents_copy {document_id, name?, folder_id?} starts a new document \
you own from the head, with its own history. documents_update {document_id, name?, \
folder_id?} renames or moves. documents_list {scope: folder | shared | mine | recent, \
folder_id?, query?, sort: name | updated | recent, limit, cursor}: shared is what others \
shared with you, recent is what you last edited, commented on or opened; each item carries \
size_bytes and my_access.";

pub(super) const PROPOSALS: &str = "\
An edit lands on the head, or is written as a proposal: a revision off the head that a \
reviewer accepts or rejects. documents_edit mode: propose always proposes; mode: land needs \
edit access. Without mode, a caller holding comment but not edit access proposes, and an \
agent proposes in a folder whose agent edit policy (or an ancestor's) is propose; there an \
agent's mode: land refuses forbidden. The ack \
carries proposal_id; revision_id is the proposed revision and written its lines against the \
base. A newer proposal of yours on the same document supersedes your open one. \
documents_proposals {document_id} lists them; documents_proposals {action: accept | reject, \
proposal_id, note?} reviews one and needs edit access; nobody accepts their own proposal \
(self_review). Accept makes the proposed revision the head when \
the head is still its base; otherwise its ops apply again on the head, and a node changed in \
between refuses as proposal_conflict.";

pub(super) const fn refusal_text(code: RefusalCode) -> &'static str {
    match code {
        RefusalCode::StaleBase => {
            "stale_base: a restore or whole-file write named a base that is no longer the head; \
             head_revision, written_by, actor_kind and at say who moved it, base_revision what you sent. The head edit wins: \
             read head_revision, re-aim at the text as it stands, never resend your old text."
        },
        RefusalCode::GuardMismatch => {
            "guard_mismatch: a node you touched changed since you read it; current_text and \
             hash are its state now. The head edit wins: re-aim at current_text with hash as \
             expected_hash, or drop the edit; never resend your old text."
        },
        RefusalCode::FindNone => {
            "find_none: old_str or a target find matched `matches` places, not one. Quote it \
             exactly as read and widen it until it is unique. For a target find that matched \
             nothing, node, hash and current_text name the closest block: re-aim at it or \
             quote its current_text."
        },
        RefusalCode::UnsupportedOp => {
            "unsupported_op: this format does not take the op or node kind; accepted_kinds lists \
             what insert_after takes. See topic formats."
        },
        RefusalCode::UnsupportedFormat => {
            "unsupported_format: no codec handles the requested format; export native or pdf \
             as url, or html as preview."
        },
        RefusalCode::TemplateFailed => {
            "template_failed: the template could not be instantiated; the text names the \
             template and the cause. Pick another template or report it."
        },
        _ => access_refusal_text(code),
    }
}

const fn access_refusal_text(code: RefusalCode) -> &'static str {
    match code {
        RefusalCode::Forbidden => {
            "forbidden: you lack the access level the action needs; the text names the level \
             held and needed, and owner names who can share it. existing_level means the \
             subject already holds more than you; leave that grant to its owner."
        },
        RefusalCode::NotFound => {
            "not_found: the document, folder, revision, comment or node does not exist or is \
             deleted; read the outline again for node ids."
        },
        RefusalCode::Trashed => {
            "trashed: the document is in the trash; documents_untrash brings it back before \
             you read, edit or share it."
        },
        RefusalCode::FolderNotEmpty => {
            "folder_not_empty: the folder still holds documents or folders; move or trash them \
             first, or delete recursively."
        },
        RefusalCode::ProposalClosed => {
            "proposal_closed: the proposal was already accepted, rejected or superseded; only an \
             open proposal can be reviewed. List open proposals again."
        },
        RefusalCode::IntegrityMismatch => {
            "integrity_mismatch: the uploaded bytes do not hash to the declared content hash; \
             the upload was corrupted, send it again."
        },
        RefusalCode::Locked => "locked: another holder has the document locked; wait and retry.",
        RefusalCode::AnchorsAffected => {
            "anchors_affected: the edit removes a node or quoted text that the listed comments \
             are anchored to. Re-aim the edit, or resend it with allow_detach: true to detach \
             them; a detached comment keeps its node."
        },
        RefusalCode::ProposalConflict => {
            "proposal_conflict: the head moved and a node the proposal touches changed; node \
             and current_text are its state at head_revision. Reject the proposal, or propose \
             again on head_revision."
        },
        RefusalCode::InvalidParams => {
            "invalid_params: the call is malformed; fix the named field and resend. An op that \
             names a node by id without expected_hash is refused this way."
        },
        RefusalCode::TooLarge => {
            "too_large: the call exceeds an input limit (ops per call, op text, markdown, \
             comment body, collaborative update or cursor); split it into smaller calls."
        },
        RefusalCode::OpIdMismatch => {
            "op_id_mismatch: this op_id was already used for a different request; send a new \
             op_id for a new write."
        },
        RefusalCode::SelfReview => {
            "self_review: you proposed this; another editor has to accept it."
        },
        RefusalCode::UnknownSubject => {
            "unknown_subject: no active user or subject has that id; check the id and resend."
        },
        RefusalCode::AccessRequestOpen => {
            "access_request_open: you already have an open request on this document; wait for \
             the owner to answer it."
        },
        RefusalCode::AccessRequestNotNeeded => {
            "access_request_not_needed: you already hold a level on this document; ask a holder \
             of share to raise it."
        },
        RefusalCode::AccessRequestClosed => {
            "access_request_closed: the request was already granted or declined; only an open \
             request can be answered."
        },
        _ => "",
    }
}
