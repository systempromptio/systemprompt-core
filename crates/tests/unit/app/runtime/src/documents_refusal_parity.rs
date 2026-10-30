use systemprompt_extension::{BrainSinkError, NotificationSinkError};
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::agents::{AgentCap, AgentCapability, AgentRefusal};
use systemprompt_files::documents::models::{ListCursorError, LockHolder};
use systemprompt_files::documents::templates::TemplateError;
use systemprompt_identifiers::{
    AgentName, ContentHash, DocumentAccessRequestId, DocumentCommentId, DocumentFolderId,
    DocumentId, DocumentNodeId, DocumentOpId, DocumentProposalId, DocumentRevisionId,
    DocumentTemplateId, EditorId,
};
use systemprompt_models::documents::{
    AccessLevel, AccessRequestStatus, DocumentAction, DocumentFormat, ProposalStatus,
};
use systemprompt_provider_contracts::documents::CodecError;
use systemprompt_runtime::documents::mcp::{refusal_result, tool_error};
use systemprompt_runtime::documents::refusal::{RefusalCode, refusal_of};

const fn listed(error: &DocumentsError) -> bool {
    match error {
        DocumentsError::Conflict { .. }
        | DocumentsError::StaleAnchor { .. }
        | DocumentsError::UnknownNode { .. }
        | DocumentsError::FindNone { .. }
        | DocumentsError::Forbidden { .. }
        | DocumentsError::Trashed { .. }
        | DocumentsError::AnchorsAffected { .. }
        | DocumentsError::InvalidComment { .. }
        | DocumentsError::InvalidAwareness { .. }
        | DocumentsError::InvalidParams { .. }
        | DocumentsError::CommentNotFound { .. }
        | DocumentsError::LockHeld { .. }
        | DocumentsError::UnsupportedOp { .. }
        | DocumentsError::UnsupportedFormat { .. }
        | DocumentsError::UnknownFormat { .. }
        | DocumentsError::NotFound { .. }
        | DocumentsError::TooLarge { .. }
        | DocumentsError::OpIdMismatch { .. }
        | DocumentsError::GuardMissing { .. }
        | DocumentsError::SelfReview { .. }
        | DocumentsError::IntegrityMismatch { .. }
        | DocumentsError::ContentRefused { .. }
        | DocumentsError::InvalidName { .. }
        | DocumentsError::UnknownSubjectKind { .. }
        | DocumentsError::UnknownSubject { .. }
        | DocumentsError::OwnNotGrantable
        | DocumentsError::ShareBelowExisting { .. }
        | DocumentsError::FolderNotEmpty { .. }
        | DocumentsError::SubtreeChanged { .. }
        | DocumentsError::OwnershipChanged { .. }
        | DocumentsError::InvalidMove { .. }
        | DocumentsError::InvalidCursor { .. }
        | DocumentsError::CursorReset { .. }
        | DocumentsError::ProposalConflict { .. }
        | DocumentsError::ProposalClosed { .. }
        | DocumentsError::AccessRequestOpen { .. }
        | DocumentsError::AccessRequestNotNeeded { .. }
        | DocumentsError::AccessRequestClosed { .. }
        | DocumentsError::Codec(_)
        | DocumentsError::Notifications(_)
        | DocumentsError::Brain(_)
        | DocumentsError::NotificationKind(_)
        | DocumentsError::Template { .. }
        | DocumentsError::LiveSession(_)
        | DocumentsError::Blob(_)
        | DocumentsError::Repository(_)
        | DocumentsError::Authz(_)
        | DocumentsError::Events { .. }
        | DocumentsError::Worker(_)
        | DocumentsError::Directory(_)
        | DocumentsError::Agent(_) => true,
    }
}

fn table() -> Vec<(DocumentsError, RefusalCode)> {
    let node = || DocumentNodeId::new("body/p[1]");
    let folder = DocumentFolderId::generate;
    vec![
        (
            DocumentsError::Conflict {
                document_id: DocumentId::generate(),
                head: DocumentRevisionId::generate(),
                base: None,
            },
            RefusalCode::StaleBase,
        ),
        (
            DocumentsError::StaleAnchor {
                node: node(),
                expected: "aaaaaaaa".to_owned(),
                actual: "bbbbbbbb".to_owned(),
                current_text: "now".to_owned(),
            },
            RefusalCode::GuardMismatch,
        ),
        (
            DocumentsError::UnknownNode { node: node() },
            RefusalCode::NotFound,
        ),
        (
            DocumentsError::FindNone {
                find: "x".to_owned(),
                closest: None,
            },
            RefusalCode::FindNone,
        ),
        (
            DocumentsError::Forbidden {
                action: DocumentAction::Edit,
                needed: AccessLevel::Edit,
                held: None,
                policy: None,
                owner: None,
            },
            RefusalCode::Forbidden,
        ),
        (
            DocumentsError::Trashed {
                document_id: DocumentId::generate(),
            },
            RefusalCode::Trashed,
        ),
        (
            DocumentsError::AnchorsAffected {
                comments: vec![DocumentCommentId::generate()],
            },
            RefusalCode::AnchorsAffected,
        ),
        (
            DocumentsError::InvalidComment { detail: "empty" },
            RefusalCode::InvalidParams,
        ),
        (
            DocumentsError::InvalidParams { detail: "missing" },
            RefusalCode::InvalidParams,
        ),
        (
            DocumentsError::InvalidAwareness {
                detail: "unknown field",
            },
            RefusalCode::InvalidParams,
        ),
        (
            DocumentsError::CommentNotFound {
                document_id: DocumentId::generate(),
                comment_id: DocumentCommentId::generate(),
            },
            RefusalCode::NotFound,
        ),
        (
            DocumentsError::LockHeld {
                document_id: DocumentId::generate(),
                holder: LockHolder::Editor(EditorId::new("h")),
            },
            RefusalCode::Locked,
        ),
        (
            DocumentsError::UnsupportedOp {
                op: "insert_after",
                format: DocumentFormat::Docx,
                accepted_kinds: &["paragraph"],
            },
            RefusalCode::UnsupportedOp,
        ),
        (
            DocumentsError::UnsupportedFormat {
                format: DocumentFormat::Docx,
            },
            RefusalCode::UnsupportedFormat,
        ),
        (
            DocumentsError::UnknownFormat {
                name: "x".to_owned(),
            },
            RefusalCode::UnsupportedFormat,
        ),
        (
            DocumentsError::NotFound {
                what: "document",
                id: "d".to_owned(),
            },
            RefusalCode::NotFound,
        ),
        (
            DocumentsError::TooLarge {
                what: "body",
                size: 2,
                limit: 1,
            },
            RefusalCode::TooLarge,
        ),
        (
            DocumentsError::OpIdMismatch {
                op_id: DocumentOpId::new("op-1"),
            },
            RefusalCode::OpIdMismatch,
        ),
        (
            DocumentsError::GuardMissing {
                op: "replace_text",
                node: node(),
            },
            RefusalCode::InvalidParams,
        ),
        (
            DocumentsError::SelfReview {
                proposal_id: DocumentProposalId::generate(),
            },
            RefusalCode::SelfReview,
        ),
        (
            DocumentsError::IntegrityMismatch {
                declared: ContentHash::new("a".repeat(64)),
                actual: ContentHash::new("b".repeat(64)),
            },
            RefusalCode::IntegrityMismatch,
        ),
        (
            DocumentsError::InvalidName {
                name: "..".to_owned(),
            },
            RefusalCode::InvalidParams,
        ),
        (
            DocumentsError::UnknownSubjectKind {
                kind: "team".to_owned(),
            },
            RefusalCode::InvalidParams,
        ),
        (
            DocumentsError::UnknownSubject {
                kind: "user".to_owned(),
                value: "u".to_owned(),
            },
            RefusalCode::UnknownSubject,
        ),
        (DocumentsError::OwnNotGrantable, RefusalCode::InvalidParams),
        (
            DocumentsError::ShareBelowExisting {
                existing: AccessLevel::Share,
                held: AccessLevel::Edit,
                owner: None,
            },
            RefusalCode::Forbidden,
        ),
        (
            DocumentsError::FolderNotEmpty {
                folder_id: folder(),
            },
            RefusalCode::FolderNotEmpty,
        ),
        (
            DocumentsError::SubtreeChanged {
                folder_id: folder(),
            },
            RefusalCode::FolderNotEmpty,
        ),
        (
            DocumentsError::OwnershipChanged {
                document_id: DocumentId::generate(),
            },
            RefusalCode::Forbidden,
        ),
        (
            DocumentsError::InvalidMove {
                folder_id: folder(),
            },
            RefusalCode::InvalidParams,
        ),
        (
            DocumentsError::ProposalConflict {
                proposal_id: DocumentProposalId::generate(),
                head: DocumentRevisionId::generate(),
                node: None,
                current_text: None,
            },
            RefusalCode::ProposalConflict,
        ),
        (
            DocumentsError::ProposalClosed {
                proposal_id: DocumentProposalId::generate(),
                status: ProposalStatus::Accepted,
            },
            RefusalCode::ProposalClosed,
        ),
        (
            DocumentsError::AccessRequestOpen {
                document_id: DocumentId::generate(),
            },
            RefusalCode::AccessRequestOpen,
        ),
        (
            DocumentsError::AccessRequestNotNeeded {
                document_id: DocumentId::generate(),
                held: AccessLevel::View,
            },
            RefusalCode::AccessRequestNotNeeded,
        ),
        (
            DocumentsError::AccessRequestClosed {
                request_id: DocumentAccessRequestId::generate(),
                status: AccessRequestStatus::Declined,
            },
            RefusalCode::AccessRequestClosed,
        ),
        (
            DocumentsError::Agent(AgentRefusal::HardRule {
                agent: AgentName::new("claude"),
                capability: AgentCapability::Share,
            }),
            RefusalCode::AgentForbidden,
        ),
        (
            DocumentsError::Agent(AgentRefusal::CapExceeded {
                agent: AgentName::new("claude"),
                cap: AgentCap::Read,
                action: "comment".to_owned(),
                reason: "capped".to_owned(),
            }),
            RefusalCode::AgentForbidden,
        ),
        (
            DocumentsError::Agent(AgentRefusal::SuggestOnly {
                agent: AgentName::new("claude"),
                reason: "capped".to_owned(),
            }),
            RefusalCode::AgentSuggestOnly,
        ),
        (
            DocumentsError::Agent(AgentRefusal::RateLimited {
                limit: 10,
                retry_after_secs: 60,
            }),
            RefusalCode::AgentRateLimited,
        ),
        (
            DocumentsError::Agent(AgentRefusal::OverrideLooser {
                requested: AgentCap::Edit,
                policy_cap: AgentCap::Read,
            }),
            RefusalCode::InvalidParams,
        ),
        (
            DocumentsError::Notifications(NotificationSinkError::Unavailable),
            RefusalCode::UnsupportedOp,
        ),
        (
            DocumentsError::Brain(BrainSinkError::Unavailable),
            RefusalCode::UnsupportedOp,
        ),
        (
            DocumentsError::CursorReset {
                cursor: "legacy".to_owned(),
                source: ListCursorError::Shape,
            },
            RefusalCode::CursorReset,
        ),
        (
            DocumentsError::Codec(CodecError::NoMatch),
            RefusalCode::FindNone,
        ),
        (
            DocumentsError::Codec(CodecError::Malformed { detail: "bad" }),
            RefusalCode::InvalidParams,
        ),
    ]
}

#[test]
fn every_client_error_maps_to_one_code_on_both_surfaces() {
    for (error, code) in table() {
        assert!(listed(&error));
        let shared = refusal_of(&error).expect("a client error refuses");
        assert_eq!(shared.refusal.code, code, "{error}");
        let mcp = refusal_result(&tool_error(error)).expect("refused tool result");
        assert_eq!(
            mcp.structured_content.expect("structured")["code"],
            code.as_str()
        );
    }
}

#[test]
fn infrastructure_failures_refuse_nothing() {
    let error = DocumentsError::Codec(CodecError::Io(std::io::Error::other("disk")));
    assert!(listed(&error));
    assert!(refusal_of(&error).is_none());
    assert!(refusal_result(&tool_error(error)).is_none());
}

#[test]
fn every_code_parses_back() {
    for code in RefusalCode::ALL {
        assert_eq!(RefusalCode::parse(code.as_str()), Some(code));
    }
}

fn template(source: TemplateError) -> DocumentsError {
    DocumentsError::Template {
        template_id: DocumentTemplateId::new("report"),
        source,
    }
}

#[test]
fn an_unknown_template_refuses_as_not_found() {
    let error = template(TemplateError::NotFound {
        id: DocumentTemplateId::new("report"),
    });
    let refused = refusal_of(&error).expect("a client error refuses");
    assert_eq!(refused.refusal.code, RefusalCode::NotFound);
}

#[test]
fn a_missing_placeholder_refuses_as_template_failed() {
    let error = template(TemplateError::MissingPlaceholder {
        key: "client".to_owned(),
    });
    let refused = refusal_of(&error).expect("a client error refuses");
    assert_eq!(refused.refusal.code, RefusalCode::TemplateFailed);
}

#[test]
fn a_template_server_fault_refuses_nothing_and_names_no_path() {
    let error = template(TemplateError::Io {
        path: std::path::PathBuf::from("/srv/services/templates/report.docx"),
        source: std::io::Error::other("gone"),
    });
    assert!(refusal_of(&error).is_none());
    assert!(refusal_result(&tool_error(error)).is_none());
    let display = template(TemplateError::Io {
        path: std::path::PathBuf::from("/srv/services/templates/report.docx"),
        source: std::io::Error::other("gone"),
    })
    .to_string();
    assert!(!display.contains("/srv/"), "{display}");
}
