//! Reopening a resolved comment clears its resolution, records who reopened
//! it, appends one `document.comment.reopened` brain fact and sends the live
//! event to the audience resolution reaches. Reopening an open comment answers
//! it as it stands and records nothing; a caller without access is refused.

use std::time::Duration;

use systemprompt_events::Broadcaster;
use systemprompt_files::documents::models::{CommentAnchor, DocumentComment};
use systemprompt_files::documents::service::{
    CommentRequest, ReopenCommentRequest, ResolveCommentRequest,
};
use systemprompt_identifiers::{ConnectionId, DocumentId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::brain::{BrainEvent, BrainEventKind, CommentReopened};
use tokio::sync::mpsc;

use crate::documents_brain_facts::{brain_facts, recording, text_document};
use crate::documents_service_fixture::{Harness, person, trace};

async fn open_comment(
    h: &Harness,
    owner: &AuthenticatedUser,
    document: &DocumentId,
) -> DocumentComment {
    let (_, projection) = h
        .service
        .project(owner, document, None, &trace())
        .await
        .expect("project");
    let node = projection
        .nodes()
        .find(|node| node.text.as_deref() == Some("alpha beta"))
        .expect("line")
        .id
        .clone();
    h.service
        .create_comment(
            owner,
            CommentRequest {
                document: document.clone(),
                anchor: Some(CommentAnchor {
                    node,
                    find: Some("beta".to_owned()),
                    nth: None,
                    cells: None,
                }),
                range: None,
                body: "settle this".to_owned(),
                mentions: Vec::new(),
                parent_id: None,
                mention_options: vec![],
            },
            &trace(),
        )
        .await
        .expect("comment")
}

async fn resolve(
    h: &Harness,
    who: &AuthenticatedUser,
    comment: &DocumentComment,
) -> DocumentComment {
    h.service
        .resolve_comment(
            who,
            ResolveCommentRequest {
                document: comment.document_id.clone(),
                comment: comment.id.clone(),
                answering: None,
            },
            &trace(),
        )
        .await
        .expect("resolve")
}

fn reopen_request(comment: &DocumentComment) -> ReopenCommentRequest {
    ReopenCommentRequest {
        document: comment.document_id.clone(),
        comment: comment.id.clone(),
    }
}

#[tokio::test]
async fn reopening_clears_the_resolution_records_the_reopener_and_appends_one_fact() {
    let h = recording().await;
    let owner = person("reopen-owner");
    let (tx, mut rx) = mpsc::channel(32);
    assert!(
        h.broadcasters
            .agui
            .register(&owner.id, &ConnectionId::generate(), tx)
            .await
    );
    let document = text_document(&h, &owner).await;
    let comment = open_comment(&h, &owner, &document).await;
    assert!(resolve(&h, &owner, &comment).await.resolved_at.is_some());

    let reopened = h
        .service
        .reopen_comment(&owner, reopen_request(&comment), &trace())
        .await
        .expect("reopen");
    assert_eq!(reopened.resolved_at, None);
    assert_eq!(reopened.resolved_by, None);
    assert!(reopened.reopened_at.is_some());
    assert_eq!(reopened.reopened_by.as_ref(), Some(&owner.id));

    let facts = brain_facts(BrainEventKind::COMMENT_REOPENED, &document).await;
    assert_eq!(facts.len(), 1, "one reopened fact");
    let event: BrainEvent = serde_json::from_value(facts[0].clone()).expect("brain event");
    let payload: CommentReopened = serde_json::from_value(event.payload).expect("payload");
    assert_eq!(payload.comment_id, comment.id);
    assert_eq!(payload.comment_author, owner.id);
    assert_eq!(payload.reopened_by, owner.id);
    assert_eq!(payload.anchor.find.as_deref(), Some("beta"));

    let mut seen = String::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
        seen.push_str(&format!("{event:?}"));
    }
    assert!(seen.contains("document.comment.reopened"), "{seen}");
}

#[tokio::test]
async fn reopening_an_open_comment_answers_it_as_it_stands_and_records_nothing() {
    let h = recording().await;
    let owner = person("reopen-open-owner");
    let document = text_document(&h, &owner).await;
    let comment = open_comment(&h, &owner, &document).await;

    let answered = h
        .service
        .reopen_comment(&owner, reopen_request(&comment), &trace())
        .await
        .expect("reopen an open comment");
    assert_eq!(answered.resolved_at, None);
    assert_eq!(
        answered.reopened_at, None,
        "an open comment is not reopened"
    );
    assert!(
        brain_facts(BrainEventKind::COMMENT_REOPENED, &document)
            .await
            .is_empty()
    );

    resolve(&h, &owner, &comment).await;
    h.service
        .reopen_comment(&owner, reopen_request(&comment), &trace())
        .await
        .expect("reopen");
    h.service
        .reopen_comment(&owner, reopen_request(&comment), &trace())
        .await
        .expect("reopen again");
    assert_eq!(
        brain_facts(BrainEventKind::COMMENT_REOPENED, &document)
            .await
            .len(),
        1,
        "a second reopen of the same resolution records nothing"
    );

    let resolved = resolve(&h, &owner, &comment).await;
    assert!(
        resolved.resolved_at.is_some(),
        "a reopened comment resolves again"
    );
    assert_eq!(resolved.reopened_by.as_ref(), Some(&owner.id));
}

#[tokio::test]
async fn a_caller_without_access_cannot_reopen() {
    let h = recording().await;
    let owner = person("reopen-guard-owner");
    let stranger = person("reopen-guard-stranger");
    let document = text_document(&h, &owner).await;
    let comment = open_comment(&h, &owner, &document).await;
    resolve(&h, &owner, &comment).await;

    let refused = h
        .service
        .reopen_comment(&stranger, reopen_request(&comment), &trace())
        .await;
    assert!(refused.is_err(), "a stranger is refused");
    let listed = h
        .service
        .comments(&owner, &document, &trace())
        .await
        .expect("list");
    assert!(
        listed[0].resolved_at.is_some(),
        "the comment stays resolved"
    );
}
