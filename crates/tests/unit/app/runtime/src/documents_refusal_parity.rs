use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::LockHolderKind;
use systemprompt_identifiers::{
    ContentHash, DocumentCommentId, DocumentFolderId, DocumentId, DocumentNodeId, DocumentOpId,
    DocumentProposalId, DocumentRevisionId,
};
use systemprompt_models::documents::{AccessLevel, DocumentAction, DocumentFormat, ProposalStatus};
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
        | DocumentsError::InvalidName { .. }
        | DocumentsError::UnknownSubjectKind { .. }
        | DocumentsError::UnknownSubject { .. }
        | DocumentsError::OwnNotGrantable
        | DocumentsError::ShareBelowExisting { .. }
        | DocumentsError::FolderNotEmpty { .. }
        | DocumentsError::InvalidMove { .. }
        | DocumentsError::InvalidCursor { .. }
        | DocumentsError::ProposalConflict { .. }
        | DocumentsError::ProposalClosed { .. }
        | DocumentsError::Codec(_)
        | DocumentsError::Template { .. }
        | DocumentsError::LiveSession(_)
        | DocumentsError::Blob(_)
        | DocumentsError::Repository(_)
        | DocumentsError::Authz(_)
        | DocumentsError::Events { .. }
        | DocumentsError::Worker(_)
        | DocumentsError::Directory(_) => true,
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
            DocumentsError::CommentNotFound {
                document_id: DocumentId::generate(),
                comment_id: DocumentCommentId::generate(),
            },
            RefusalCode::NotFound,
        ),
        (
            DocumentsError::LockHeld {
                document_id: DocumentId::generate(),
                holder_kind: LockHolderKind::Editor,
                holder_id: "h".to_owned(),
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
