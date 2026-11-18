//! The `documents_guide` explanation of each refusal code, served as
//! `refusal.<code>`; re-exported through `guide_topics`.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use crate::documents::refusal::RefusalCode;

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
             what insert_after takes. A file document (PDF, image, any upload) takes no op at \
             all. See topic formats."
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
        _ => agent_refusal_text(code),
    }
}

const fn agent_refusal_text(code: RefusalCode) -> &'static str {
    match code {
        RefusalCode::AgentForbidden => {
            "agent_forbidden: your user may do this but agents may not here: the document's \
             agent cap is below the action, or the action is one agents never take. Ask your \
             user. See topic agents."
        },
        RefusalCode::AgentSuggestOnly => {
            "agent_suggest_only: agents may only suggest on this document; resend the change \
             with mode: propose and a person reviews it."
        },
        RefusalCode::AgentRateLimited => {
            "agent_rate_limited: your user's agents have made the most edits allowed this \
             hour; wait the seconds the message names, then retry with the same op_id."
        },
        RefusalCode::CursorReset => {
            "cursor_reset: the Drive cursor sent is not a position of the current change \
             feed; drop it, read the manifest again and continue from its cursor."
        },
        RefusalCode::NameTaken => {
            "name_taken: the folder already holds a document or folder of that name; choose \
             another name, or edit the existing document or use the existing folder."
        },
        _ => "",
    }
}
