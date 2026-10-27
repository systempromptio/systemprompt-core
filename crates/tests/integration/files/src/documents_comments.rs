use std::time::Duration;

use systemprompt_events::Broadcaster;
use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::CommentAnchor;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommentRequest, CommitOutcome, CommitRequest, ResolveCommentRequest,
};
use systemprompt_identifiers::{ConnectionId, DocumentCommentId, DocumentId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    ContentAs, EditOp, NodeKind, NodeTarget, Projection, ProjectionNode,
};
use tokio::sync::mpsc;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, create, harness, person, trace,
};

async fn text_document(h: &Harness, owner: &AuthenticatedUser, text: &str) -> DocumentId {
    let mut request = create("blank-text", None);
    request.ops = vec![EditOp::SetText {
        text: text.to_owned(),
    }];
    h.service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create")
        .0
        .id
}

async fn head(
    h: &Harness,
    owner: &AuthenticatedUser,
    document: &DocumentId,
) -> (DocumentRevisionId, Projection) {
    let (revision, projection) = h
        .service
        .project(owner, document, None, &trace())
        .await
        .expect("project");
    (revision.id, projection)
}

fn line<'p>(projection: &'p Projection, text: &str) -> &'p ProjectionNode {
    projection
        .nodes()
        .find(|node| node.text.as_deref() == Some(text))
        .unwrap_or_else(|| panic!("no line {text}"))
}

fn ops(document: &DocumentId, ops: Vec<EditOp>, comments: CommentPolicy) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: None,
        body: CommitBody::Ops(ops),
        author: api_author(),
        message: None,
        lock_holder_id: None,
        comments,
    }
}

fn replace(node: &ProjectionNode, text: &str) -> EditOp {
    EditOp::ReplaceText {
        node: NodeTarget::Id(node.id.clone()),
        expected_hash: Some(node.hash8.clone()),
        text: text.to_owned(),
        content_as: ContentAs::Text,
    }
}

async fn comment_on(
    h: &Harness,
    owner: &AuthenticatedUser,
    document: &DocumentId,
    node: &ProjectionNode,
    find: Option<&str>,
) -> DocumentCommentId {
    h.service
        .create_comment(
            owner,
            CommentRequest {
                document: document.clone(),
                anchor: Some(CommentAnchor {
                    node: node.id.clone(),
                    find: find.map(str::to_owned),
                    nth: None,
                }),
                body: "check this".to_owned(),
                parent_id: None,
            },
            &trace(),
        )
        .await
        .expect("comment")
        .id
}

fn revision_of(outcome: CommitOutcome) -> DocumentRevisionId {
    match outcome {
        CommitOutcome::Committed { revision, .. } | CommitOutcome::Merged { revision, .. } => {
            revision.id
        },
        CommitOutcome::Conflict { head, .. } => panic!("conflict at {}", head.id),
    }
}

#[tokio::test]
async fn comments_are_created_listed_replied_to_and_resolved_with_events() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("comment-owner");
    let (tx, mut rx) = mpsc::channel(32);
    assert!(
        h.broadcasters
            .agui
            .register(&owner.id, &ConnectionId::generate(), tx)
            .await
    );
    let document = text_document(&h, &owner, "alpha beta\nsecond line\n").await;
    let (_, projection) = head(&h, &owner, &document).await;
    let node = line(&projection, "alpha beta").clone();
    let root = comment_on(&h, &owner, &document, &node, Some("beta")).await;
    let reply = h
        .service
        .create_comment(
            &owner,
            CommentRequest {
                document: document.clone(),
                anchor: None,
                body: "agreed".to_owned(),
                parent_id: Some(root.clone()),
            },
            &trace(),
        )
        .await
        .expect("reply");
    assert_eq!(reply.anchor.node, node.id, "a reply carries its parent's anchor");
    assert_eq!(reply.parent_id.as_ref(), Some(&root));

    let resolved = h
        .service
        .resolve_comment(
            &owner,
            ResolveCommentRequest {
                document: document.clone(),
                comment: root.clone(),
                answering: None,
            },
            &trace(),
        )
        .await
        .expect("resolve");
    assert!(resolved.resolved_at.is_some());
    assert_eq!(resolved.resolved_by.as_ref(), Some(&owner.id));

    let listed = h
        .service
        .comments(&owner, &document, &trace())
        .await
        .expect("list");
    assert_eq!(listed.len(), 2);
    assert_eq!(listed[0].anchor.find.as_deref(), Some("beta"));

    let mut seen = String::new();
    while let Ok(Some(event)) = tokio::time::timeout(Duration::from_millis(200), rx.recv()).await {
        seen.push_str(&format!("{event:?}"));
    }
    assert!(seen.contains("document.comment.created"), "{seen}");
    assert!(seen.contains("document.comment.resolved"), "{seen}");
}

#[tokio::test]
async fn a_comment_needs_a_quote_the_node_contains() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("comment-quote");
    let document = text_document(&h, &owner, "alpha beta\n").await;
    let (_, projection) = head(&h, &owner, &document).await;
    let node = line(&projection, "alpha beta").clone();
    let refused = h
        .service
        .create_comment(
            &owner,
            CommentRequest {
                document: document.clone(),
                anchor: Some(CommentAnchor {
                    node: node.id,
                    find: Some("gamma".to_owned()),
                    nth: None,
                }),
                body: "x".to_owned(),
                parent_id: None,
            },
            &trace(),
        )
        .await
        .expect_err("quote is absent");
    assert!(matches!(refused, DocumentsError::FindNone { .. }), "{refused:?}");
}

#[tokio::test]
async fn an_edit_removing_anchored_words_refuses_until_detach_is_allowed() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("comment-anchor");
    let document = text_document(&h, &owner, "alpha beta\nsecond line\n").await;
    let (_, projection) = head(&h, &owner, &document).await;
    let node = line(&projection, "alpha beta").clone();
    let comment = comment_on(&h, &owner, &document, &node, Some("beta")).await;

    let refused = h
        .service
        .commit(
            &owner,
            ops(&document, vec![replace(&node, "alpha gamma")], CommentPolicy::default()),
            &trace(),
        )
        .await
        .expect_err("anchored words removed");
    let DocumentsError::AnchorsAffected { comments } = refused else {
        panic!("expected anchors_affected, got {refused:?}");
    };
    assert_eq!(comments, vec![comment.clone()]);

    h.service
        .commit(
            &owner,
            ops(&document, vec![replace(&node, "alpha gamma")], CommentPolicy::detach()),
            &trace(),
        )
        .await
        .expect("detach allowed");
    let listed = h
        .service
        .comments(&owner, &document, &trace())
        .await
        .expect("list");
    assert_eq!(listed[0].anchor.node, node.id, "a detached comment keeps its node");
    assert!(listed[0].anchor.find.is_none(), "a detached comment loses its quote");
}

#[tokio::test]
async fn an_anchor_survives_an_insert_before_it() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("comment-survive");
    let document = text_document(&h, &owner, "alpha beta\nsecond line\n").await;
    let (_, projection) = head(&h, &owner, &document).await;
    let body = projection.root[0].clone();
    let node = line(&projection, "second line").clone();
    comment_on(&h, &owner, &document, &node, Some("line")).await;
    let insert = EditOp::InsertAfter {
        node: NodeTarget::Id(body.id.clone()),
        kind: NodeKind::Text,
        text: "new first line".to_owned(),
        content_as: ContentAs::Text,
    };
    h.service
        .commit(&owner, ops(&document, vec![insert], CommentPolicy::default()), &trace())
        .await
        .expect("an insert elsewhere lands");
    let (_, after) = head(&h, &owner, &document).await;
    let moved = line(&after, "second line");
    assert_eq!(moved.id, node.id, "the anchored node keeps its id");
    let listed = h
        .service
        .comments(&owner, &document, &trace())
        .await
        .expect("list");
    assert_eq!(listed[0].anchor.find.as_deref(), Some("line"));
}

#[tokio::test]
async fn an_edit_answering_a_comment_records_its_revision() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("comment-answer");
    let document = text_document(&h, &owner, "alpha beta\nsecond line\n").await;
    let (_, projection) = head(&h, &owner, &document).await;
    let node = line(&projection, "alpha beta").clone();
    let comment = comment_on(&h, &owner, &document, &node, None).await;
    let other = line(&projection, "second line").clone();
    let outcome = h
        .service
        .commit(
            &owner,
            ops(
                &document,
                vec![replace(&other, "second line, revised")],
                CommentPolicy {
                    allow_detach: false,
                    answering: Some(comment.clone()),
                },
            ),
            &trace(),
        )
        .await
        .expect("answering edit");
    let revision = revision_of(outcome);
    let listed = h
        .service
        .comments(&owner, &document, &trace())
        .await
        .expect("list");
    assert_eq!(listed[0].answering_revision_id.as_ref(), Some(&revision));
}
