//! The revision engine's landing transaction: node id counters on the
//! primary, landed-only history, op id acknowledgements claimed with the
//! commit, required guards on id targets, and input limits.

use std::collections::HashSet;

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::models::{NewRevision, RevisionAttribution};
use systemprompt_files::documents::repository::DocumentNodeIdRepository;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    Author, CommentPolicy, CommentRequest, CommitOutcome, CommitRequest, HistoryPage, LandedCommit,
    RevisionOrigin,
};
use systemprompt_identifiers::{
    DocumentId, DocumentNodeId, DocumentOpId, DocumentRevisionId, SessionId,
};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AccessLevel, ActorKind, ContentAs, EditOp, NodeKind, NodeTarget,
};
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_proposals::{grant, memo, outline, propose, rewrite};
use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, harness, person, trace, unique,
};

fn request(document: &DocumentId, ops: Vec<EditOp>, author: Author) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: None,
        body: CommitBody::Ops(ops),
        author,
        message: None,
        lock_holder_id: None,
        comments: CommentPolicy::default(),
        op_id: None,
    }
}

fn landed(outcome: CommitOutcome) -> LandedCommit {
    match outcome {
        CommitOutcome::Committed(landed) | CommitOutcome::Merged(landed) => *landed,
        CommitOutcome::Conflict { head, .. } => panic!("conflict at {}", head.id),
    }
}

fn insert_after(node: &DocumentNodeId, text: &str) -> EditOp {
    EditOp::InsertAfter {
        node: NodeTarget::Id(node.clone()),
        kind: NodeKind::Paragraph,
        text: text.to_owned(),
        content_as: ContentAs::Text,
    }
}

async fn commit(
    h: &Harness,
    who: &AuthenticatedUser,
    request: CommitRequest,
) -> Result<CommitOutcome, DocumentsError> {
    h.service.commit(who, request, &trace()).await
}

async fn unlanded_rows(document: &DocumentId) -> i64 {
    let pool = test_db_pool().await.pool();
    sqlx::query_scalar::<_, i64>(
        "SELECT COUNT(*) FROM document_revisions
         WHERE document_id = $1::uuid AND landed_at IS NULL",
    )
    .bind(document.as_str())
    .fetch_one(&*pool)
    .await
    .expect("count unlanded")
}

#[tokio::test]
async fn counter_reservations_never_overlap() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("counter");
    let (document, _) = memo(&h, &owner, None).await;
    let reserve = || async {
        let mut tx = h.repos.documents.begin().await.expect("begin");
        let first = DocumentNodeIdRepository::reserve(&mut tx, &document, "w", 10)
            .await
            .expect("reserve");
        tx.commit().await.expect("commit");
        first
    };
    let (a, b) = tokio::join!(reserve(), reserve());
    assert_eq!((a - b).abs(), 10, "ranges {a}.. and {b}.. overlap");
}

#[tokio::test]
async fn concurrent_commits_from_one_session_mint_distinct_ids() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("same-session");
    let (document, _) = memo(&h, &owner, None).await;
    let head = outline(&h, &owner, &document, None).await;
    let anchors: Vec<DocumentNodeId> = head
        .nodes()
        .filter(|n| n.text.is_some())
        .map(|n| n.id.clone())
        .collect();
    let session = Author {
        actor_kind: ActorKind::User,
        origin: RevisionOrigin::Api,
        attribution: RevisionAttribution {
            session_id: Some(SessionId::new(unique("session"))),
            ..RevisionAttribution::default()
        },
    };
    let first = request(
        &document,
        vec![insert_after(&anchors[0], "one")],
        session.clone(),
    );
    let second = request(&document, vec![insert_after(&anchors[2], "two")], session);
    let (a, b) = tokio::join!(commit(&h, &owner, first), commit(&h, &owner, second));
    let mut created = HashSet::new();
    for outcome in [a.expect("first"), b.expect("second")] {
        if let Some(landed) = outcome.landed() {
            for id in &landed.ack.created {
                assert!(created.insert(id.clone()), "id {id} minted twice");
            }
        }
    }
    assert!(!created.is_empty());
    let after = outline(&h, &owner, &document, None).await;
    let ids: Vec<_> = after.nodes().map(|n| n.id.clone()).collect();
    assert_eq!(ids.iter().collect::<HashSet<_>>().len(), ids.len());
    assert!(ids.iter().all(|id| id.as_str().contains('.')), "{ids:?}");
    assert_eq!(unlanded_rows(&document).await, 0);
}

#[tokio::test]
async fn an_unlanded_revision_is_not_history_and_cannot_be_read_or_restored() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("orphan");
    let (document, base) = memo(&h, &owner, None).await;
    let head = h
        .service
        .revision(&owner, &document, &base, &trace())
        .await
        .expect("head");
    let orphan = h
        .repos
        .revisions
        .insert(NewRevision {
            id: DocumentRevisionId::generate(),
            document_id: document.clone(),
            parent_revision_id: Some(base.clone()),
            content_sha256: head.content_sha256.clone(),
            author_user_id: owner.id.clone(),
            actor_kind: ActorKind::User,
            origin_kind: systemprompt_models::documents::RevisionOriginKind::Api,
            origin_id: None,
            message: None,
            edit_ops: None,
            attribution: RevisionAttribution::default(),
        })
        .await
        .expect("orphan row");
    let history = h
        .service
        .history(
            &owner,
            &document,
            HistoryPage {
                limit: 50,
                before: None,
            },
            &trace(),
        )
        .await
        .expect("history");
    assert!(history.iter().all(|r| r.id != orphan.id));
    assert!(history.iter().all(|r| r.landed_at.is_some()));
    let read = h
        .service
        .project(&owner, &document, Some(&orphan.id), &trace())
        .await;
    assert!(
        matches!(read, Err(DocumentsError::NotFound { .. })),
        "{read:?}"
    );
    let restore = commit(
        &h,
        &owner,
        CommitRequest {
            base_revision: Some(base),
            ..request(
                &document,
                vec![EditOp::Restore {
                    revision_id: orphan.id.clone(),
                }],
                api_author(),
            )
        },
    )
    .await;
    assert!(
        matches!(restore, Err(DocumentsError::NotFound { .. })),
        "{restore:?}"
    );
}

#[tokio::test]
async fn racing_commits_leave_no_unlanded_revision() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("race");
    let (document, _) = memo(&h, &owner, None).await;
    let head = outline(&h, &owner, &document, None).await;
    let edits = ["alpha", "beta", "gamma"].map(|text| {
        request(
            &document,
            vec![rewrite(&head, text, &format!("{text} edited"))],
            api_author(),
        )
    });
    let [a, b, c] = edits;
    let (a, b, c) = tokio::join!(
        commit(&h, &owner, a),
        commit(&h, &owner, b),
        commit(&h, &owner, c)
    );
    let landed_count = [a, b, c]
        .into_iter()
        .filter(|o| o.as_ref().expect("commit").landed().is_some())
        .count();
    assert!(landed_count >= 1);
    assert_eq!(unlanded_rows(&document).await, 0);
    let history = h
        .service
        .history(
            &owner,
            &document,
            HistoryPage {
                limit: 50,
                before: None,
            },
            &trace(),
        )
        .await
        .expect("history");
    assert_eq!(history.len(), landed_count + 1);
}

#[tokio::test]
async fn since_revision_refuses_a_proposed_revision() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("since-owner");
    let proposer = person("since-proposer");
    let (document, _) = memo(&h, &owner, None).await;
    grant(&h, &owner, &document, &proposer, AccessLevel::Comment).await;
    let head = outline(&h, &owner, &document, None).await;
    let proposed = propose(
        &h,
        &proposer,
        &document,
        vec![rewrite(&head, "beta", "BETA")],
    )
    .await
    .expect("propose");
    assert!(proposed.revision.landed_at.is_none());
    let since = h
        .service
        .project(&owner, &document, Some(&proposed.revision.id), &trace())
        .await;
    assert!(
        matches!(since, Err(DocumentsError::NotFound { .. })),
        "{since:?}"
    );
}

#[tokio::test]
async fn an_op_id_applies_once_and_replays_under_concurrency() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("op-once");
    let (document, _) = memo(&h, &owner, None).await;
    let head = outline(&h, &owner, &document, None).await;
    let op_id = DocumentOpId::new(unique("op"));
    let same = || CommitRequest {
        op_id: Some(op_id.clone()),
        ..request(
            &document,
            vec![rewrite(&head, "beta", "BETA")],
            api_author(),
        )
    };
    let (a, b) = tokio::join!(commit(&h, &owner, same()), commit(&h, &owner, same()));
    let (a, b) = (landed(a.expect("a")), landed(b.expect("b")));
    assert_eq!(a.revision.id, b.revision.id);
    assert_eq!(a.ack, b.ack);
    assert!(a.replayed != b.replayed, "exactly one call applies");
    let again = landed(commit(&h, &owner, same()).await.expect("retry"));
    assert!(again.replayed);
    assert!(again.projection.is_none());
    let history = h
        .service
        .history(
            &owner,
            &document,
            HistoryPage {
                limit: 50,
                before: None,
            },
            &trace(),
        )
        .await
        .expect("history");
    assert_eq!(history.len(), 2);

    let different = CommitRequest {
        op_id: Some(op_id.clone()),
        ..request(
            &document,
            vec![rewrite(&head, "gamma", "GAMMA")],
            api_author(),
        )
    };
    let mismatch = commit(&h, &owner, different).await;
    assert!(
        matches!(mismatch, Err(DocumentsError::OpIdMismatch { .. })),
        "{mismatch:?}"
    );

    let editor = person("op-other");
    grant(&h, &owner, &document, &editor, AccessLevel::Edit).await;
    let foreign = commit(&h, &editor, same()).await;
    assert!(
        matches!(foreign, Err(DocumentsError::OpIdMismatch { .. })),
        "{foreign:?}"
    );
}

#[tokio::test]
async fn an_id_target_without_expected_hash_is_refused() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("no-hash");
    let (document, _) = memo(&h, &owner, None).await;
    let head = outline(&h, &owner, &document, None).await;
    let EditOp::ReplaceText { node, text, .. } = rewrite(&head, "beta", "BETA") else {
        panic!("rewrite builds replace_text");
    };
    let unguarded = EditOp::ReplaceText {
        node,
        expected_hash: None,
        text,
        content_as: ContentAs::Text,
    };
    let refused = commit(
        &h,
        &owner,
        request(&document, vec![unguarded], api_author()),
    )
    .await;
    assert!(
        matches!(refused, Err(DocumentsError::GuardMissing { .. })),
        "{refused:?}"
    );
    let quoted = EditOp::ReplaceText {
        node: NodeTarget::Find(systemprompt_models::documents::QuoteTarget {
            find: "beta".to_owned(),
            nth: None,
            within: None,
        }),
        expected_hash: None,
        text: "BETA".to_owned(),
        content_as: ContentAs::Text,
    };
    landed(
        commit(&h, &owner, request(&document, vec![quoted], api_author()))
            .await
            .expect("a quote target resolves its own hash"),
    );
}

#[tokio::test]
async fn inputs_over_the_limits_are_refused() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("limits");
    let (document, _) = memo(&h, &owner, None).await;
    let head = outline(&h, &owner, &document, None).await;
    let anchor = head
        .nodes()
        .find(|n| n.text.is_some())
        .expect("block")
        .id
        .clone();
    let many = (0..201)
        .map(|n| insert_after(&anchor, &n.to_string()))
        .collect();
    let too_many = commit(&h, &owner, request(&document, many, api_author())).await;
    assert!(
        matches!(too_many, Err(DocumentsError::TooLarge { .. })),
        "{too_many:?}"
    );
    let markdown = EditOp::AppendMarkdown {
        markdown: "x".repeat(1024 * 1024 + 1),
    };
    let too_long = commit(&h, &owner, request(&document, vec![markdown], api_author())).await;
    assert!(
        matches!(too_long, Err(DocumentsError::TooLarge { .. })),
        "{too_long:?}"
    );
    let comment = h
        .service
        .create_comment(
            &owner,
            CommentRequest {
                document: document.clone(),
                anchor: None,
                parent_id: None,
                body: "c".repeat(16 * 1024 + 1),
            },
            &trace(),
        )
        .await;
    assert!(
        matches!(comment, Err(DocumentsError::TooLarge { .. })),
        "{comment:?}"
    );
}
