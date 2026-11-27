use serde_json::json;
use systemprompt_drive::{ConflictHead, DriveError, NeededCapability};
use systemprompt_harness::members::drive::agent::{
    ProposeEdit, failure_text, propose_edit, search_documents,
};
use systemprompt_models::documents::client::DriveScope;

use crate::members::drive::support::{
    DOC, FakeInstance, FakeResponse, REV, USER, document_id, revision_id,
};

#[test]
fn failures_read_as_next_steps() {
    let suggest = DriveError::AgentRefused {
        endpoint: "POST /x",
        error_key: "agent_suggest_only".to_owned(),
        capability: NeededCapability::Edit,
    };
    assert!(failure_text(&suggest).contains("proposal"));
    let conflict = DriveError::Conflict {
        endpoint: "POST /x",
        error_key: Some("not_found".to_owned()),
        head: ConflictHead {
            head_revision_id: Some(revision_id(REV)),
            ..ConflictHead::default()
        },
    };
    let text = failure_text(&conflict);
    assert!(text.contains(REV));
    assert!(text.contains("node"));
    assert!(failure_text(&DriveError::Cancelled).contains("cancelled"));
}

#[tokio::test]
async fn listings_are_bounded() {
    let documents: Vec<serde_json::Value> = (0..60)
        .map(|n| {
            json!({
                "id": format!("00000000-0000-4000-8000-{n:012}"),
                "name": format!("doc {n}"),
                "format": "markdown",
                "mime_type": "text/markdown",
                "head_revision_id": REV,
                "owner_user_id": USER,
                "created_at": "t",
                "updated_at": "t"
            })
        })
        .collect();
    let fake =
        FakeInstance::start(move |_| FakeResponse::json(200, &json!({ "documents": documents })))
            .await;
    let found = search_documents(&fake.drive(), "doc", DriveScope::Mine)
        .await
        .unwrap();
    assert_eq!(found.data.len(), 60);
    assert!(found.text.contains("[10 more not shown]"));
    assert_eq!(found.text.lines().count(), 52);
    let sent = fake.last();
    assert_eq!(sent.path(), "/api/v1/documents");
    assert!(sent.query().contains("query=doc"));
}

#[tokio::test]
async fn an_edit_without_operations_is_not_sent() {
    let fake = FakeInstance::start(|_| FakeResponse::json(200, &json!({}))).await;
    let edit = ProposeEdit {
        id: document_id(DOC),
        ops: Vec::new(),
        base_revision: Some(revision_id(REV)),
        message: None,
        context_id: None,
    };
    let refused = propose_edit(&fake.drive(), edit).await.unwrap_err();
    assert!(matches!(refused, DriveError::InvalidInput { .. }));
    assert!(fake.requests().is_empty());
}
