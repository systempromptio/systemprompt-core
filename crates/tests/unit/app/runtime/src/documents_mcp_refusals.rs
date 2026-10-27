use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::templates::TemplateError;
use systemprompt_identifiers::{
    DocumentId, DocumentNodeId, DocumentRevisionId, DocumentTemplateId,
};
use systemprompt_mcp::McpError;
use systemprompt_models::documents::DocumentFormat;
use systemprompt_runtime::documents::mcp::{RefusalCode, refusal_result, tool_error};
use systemprompt_traits::RepositoryError;

fn refused(error: DocumentsError) -> (String, serde_json::Value) {
    let mcp = tool_error(error);
    let result = refusal_result(&mcp).expect("a refusal is a tool result");
    assert_eq!(result.is_error, Some(true));
    let text = serde_json::to_value(&result.content[0])
        .ok()
        .and_then(|v| v.get("text").and_then(|t| t.as_str()).map(str::to_owned))
        .expect("text block");
    (text, result.structured_content.expect("structured refusal"))
}

#[test]
fn stale_base_carries_the_head_revision() {
    let head = DocumentRevisionId::generate();
    let (text, data) = refused(DocumentsError::Conflict {
        document_id: DocumentId::generate(),
        head: head.clone(),
        base: None,
    });
    assert_eq!(data["code"], "stale_base");
    assert_eq!(data["head_revision"], head.as_str());
    assert!(text.contains(head.as_str()), "{text}");
}

#[test]
fn guard_mismatch_carries_the_current_text() {
    let (_, data) = refused(DocumentsError::StaleAnchor {
        node: DocumentNodeId::new("body/p[1]"),
        expected: "aaaaaaaa".to_owned(),
        actual: "bbbbbbbb".to_owned(),
        current_text: "as it stands".to_owned(),
    });
    assert_eq!(data["code"], "guard_mismatch");
    assert_eq!(data["current_text"], "as it stands");
    assert_eq!(data["hash"], "bbbbbbbb");
    assert_eq!(data["node"], "body/p[1]");
}

#[test]
fn unsupported_op_names_the_accepted_kinds() {
    let (text, data) = refused(DocumentsError::UnsupportedOp {
        op: "insert_after",
        format: DocumentFormat::Docx,
        accepted_kinds: &["heading", "paragraph", "list_item"],
    });
    assert_eq!(data["code"], "unsupported_op");
    assert_eq!(
        data["accepted_kinds"],
        serde_json::json!(["heading", "paragraph", "list_item"])
    );
    assert!(text.contains("heading, paragraph, list_item"), "{text}");
}

#[test]
fn template_failure_names_the_template_and_its_cause() {
    let id = DocumentTemplateId::new("astound-word");
    let (text, data) = refused(DocumentsError::Template {
        template_id: id.clone(),
        source: TemplateError::NotFound { id },
    });
    assert_eq!(data["code"], "template_failed");
    assert!(text.contains("astound-word"), "{text}");
    assert!(text.contains("does not exist"), "{text}");
}

#[test]
fn a_missing_quote_is_find_none() {
    use systemprompt_provider_contracts::documents::CodecError;
    let (_, data) = refused(DocumentsError::Codec(CodecError::NoMatch));
    assert_eq!(data["code"], "find_none");
    assert_eq!(data["matches"], 0);
    let (_, data) = refused(DocumentsError::Codec(CodecError::AmbiguousMatch {
        matches: 3,
    }));
    assert_eq!(data["code"], "find_none");
    assert_eq!(data["matches"], 3);
}

#[test]
fn infrastructure_failures_stay_protocol_errors() {
    let error = tool_error(DocumentsError::Repository(RepositoryError::NotFound {
        entity: "row",
        key: None,
    }));
    assert!(refusal_result(&error).is_none());
}

#[test]
fn untagged_invalid_params_become_invalid_params_refusals() {
    let result =
        refusal_result(&McpError::invalid_params("missing field `ops`", None)).expect("refusal");
    assert_eq!(result.is_error, Some(true));
    assert_eq!(
        result.structured_content.expect("structured")["code"],
        "invalid_params"
    );
}

#[test]
fn every_code_serialises_as_its_name() {
    for code in RefusalCode::ALL {
        assert_eq!(
            serde_json::to_value(code).expect("code"),
            serde_json::Value::String(code.as_str().to_owned())
        );
    }
}

#[test]
fn forbidden_names_the_owner_and_trash_and_anchors_carry_detail() {
    let owner = systemprompt_identifiers::UserId::new("owner-1");
    let (text, data) = refused(DocumentsError::Forbidden {
        action: systemprompt_models::documents::DocumentAction::Edit,
        needed: systemprompt_models::documents::AccessLevel::Edit,
        held: Some(systemprompt_models::documents::AccessLevel::View),
        policy: None,
        owner: Some(owner.clone()),
    });
    assert_eq!(data["code"], "forbidden");
    assert_eq!(data["owner"], "owner-1");
    assert!(text.contains("owner-1"), "{text}");

    let (_, data) = refused(DocumentsError::Trashed {
        document_id: DocumentId::generate(),
    });
    assert_eq!(data["code"], "not_found");
    assert_eq!(data["trashed"], true);

    let comment = systemprompt_identifiers::DocumentCommentId::generate();
    let (text, data) = refused(DocumentsError::AnchorsAffected {
        comments: vec![comment.clone()],
    });
    assert_eq!(data["code"], "anchors_affected");
    assert_eq!(data["comments"][0], comment.as_str());
    assert!(text.contains("allow_detach"), "{text}");
}

#[test]
fn proposal_conflict_carries_the_node_and_its_current_text() {
    let head = DocumentRevisionId::generate();
    let (text, data) = refused(DocumentsError::ProposalConflict {
        proposal_id: systemprompt_identifiers::DocumentProposalId::generate(),
        head: head.clone(),
        node: Some(DocumentNodeId::new("n-7")),
        current_text: Some("owner wins".to_owned()),
    });
    assert_eq!(data["code"], "proposal_conflict");
    assert_eq!(data["head_revision"], head.as_str());
    assert_eq!(data["node"], "n-7");
    assert_eq!(data["current_text"], "owner wins");
    assert!(text.contains("owner wins"), "{text}");
}

#[test]
fn share_below_existing_names_the_existing_level() {
    let (_, data) = refused(DocumentsError::ShareBelowExisting {
        existing: systemprompt_models::documents::AccessLevel::Own,
        held: systemprompt_models::documents::AccessLevel::Share,
        owner: None,
    });
    assert_eq!(data["code"], "forbidden");
    assert_eq!(data["existing_level"], "own");
}

#[test]
fn unknown_subject_and_own_grants_have_their_codes() {
    let (_, data) = refused(DocumentsError::UnknownSubject {
        kind: "user".to_owned(),
        value: "ghost".to_owned(),
    });
    assert_eq!(data["code"], "unknown_subject");
    let (_, data) = refused(DocumentsError::OwnNotGrantable);
    assert_eq!(data["code"], "invalid_params");
}
