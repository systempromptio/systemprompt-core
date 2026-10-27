//! The shared-editing model: stable node ids carried across revisions,
//! node-scoped guards on edits and on stale Office uploads, restore and
//! re-insert from history, and the attribution a revision records.

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::codecs::docx::DocxCodec;
use systemprompt_files::documents::models::RevisionAttribution;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    Author, CommentPolicy, CommitOutcome, CommitRequest, HistoryPage, RevisionOrigin,
};
use systemprompt_identifiers::{
    ContextId, DocumentId, DocumentNodeId, DocumentRevisionId, SessionId, TaskId, TraceId,
};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    ActorKind, ContentAs, EditOp, NodeKind, NodeTarget, Projection, ProjectionNode,
};
use systemprompt_provider_contracts::documents::DocumentCodec;
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, create, harness, person, trace,
};

async fn memo(h: &Harness, owner: &AuthenticatedUser) -> (DocumentId, DocumentRevisionId) {
    let mut request = create("blank-docx", None);
    request.ops = vec![EditOp::AppendMarkdown {
        markdown: "alpha\n\nbeta\n\ngamma".to_owned(),
    }];
    let (document, revision) = h
        .service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create memo");
    (document.id, revision.id)
}

async fn ids(
    h: &Harness,
    owner: &AuthenticatedUser,
    document: &DocumentId,
    revision: &DocumentRevisionId,
) -> Projection {
    h.service
        .project(owner, document, Some(revision), &trace())
        .await
        .expect("project")
        .1
}

fn block<'p>(projection: &'p Projection, text: &str) -> &'p ProjectionNode {
    projection
        .nodes()
        .find(|node| node.children.is_empty() && node.text.as_deref() == Some(text))
        .unwrap_or_else(|| panic!("no block {text}"))
}

fn texts(projection: &Projection) -> Vec<String> {
    projection
        .nodes()
        .filter(|node| node.children.is_empty())
        .filter_map(|node| node.text.clone())
        .filter(|text| !text.is_empty())
        .collect()
}

fn replace(node: &ProjectionNode, text: &str) -> EditOp {
    EditOp::ReplaceText {
        node: NodeTarget::Id(node.id.clone()),
        expected_hash: Some(node.hash8.clone()),
        text: text.to_owned(),
        content_as: ContentAs::Text,
    }
}

fn ops_on(
    document: &DocumentId,
    base: Option<&DocumentRevisionId>,
    ops: Vec<EditOp>,
) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: base.cloned(),
        body: CommitBody::Ops(ops),
        author: api_author(),
        message: None,
        lock_holder_id: None,
        comments: CommentPolicy::default(),
        op_id: None,
    }
}

async fn commit(
    h: &Harness,
    owner: &AuthenticatedUser,
    request: CommitRequest,
) -> Result<CommitOutcome, DocumentsError> {
    h.service.commit(owner, request, &trace()).await
}

fn landed(outcome: CommitOutcome) -> (DocumentRevisionId, bool) {
    match outcome {
        CommitOutcome::Committed(landed) => (landed.revision.id, false),
        CommitOutcome::Merged(landed) => (landed.revision.id, true),
        CommitOutcome::Conflict { head, .. } => panic!("conflict at head {}", head.id),
    }
}

#[tokio::test]
async fn ids_survive_an_insert_above_and_a_delete() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("ids");
    let (document, first) = memo(&h, &owner).await;
    let before = ids(&h, &owner, &document, &first).await;
    let (alpha, beta, gamma) = (
        block(&before, "alpha").clone(),
        block(&before, "beta").clone(),
        block(&before, "gamma").clone(),
    );
    let inserted = EditOp::InsertAfter {
        node: NodeTarget::Id(alpha.id.clone()),
        kind: NodeKind::Paragraph,
        text: "inserted".to_owned(),
        content_as: ContentAs::Text,
    };
    let delete = EditOp::Delete {
        node: NodeTarget::Id(beta.id.clone()),
        expected_hash: Some(beta.hash8.clone()),
    };
    let outcome = commit(
        &h,
        &owner,
        ops_on(&document, Some(&first), vec![inserted, delete]),
    )
    .await
    .expect("insert and delete");
    let (second, merged) = landed(outcome);
    assert!(!merged);
    let after = ids(&h, &owner, &document, &second).await;
    assert_eq!(block(&after, "alpha").id, alpha.id);
    assert_eq!(block(&after, "gamma").id, gamma.id);
    let new = block(&after, "inserted");
    assert!(before.find(&new.id).is_none(), "minted {}", new.id);
    assert!(after.find(&beta.id).is_none(), "retired {}", beta.id);
    assert_eq!(
        new.attrs.get("path"),
        beta.attrs.get("path"),
        "the inserted block took the deleted block's position, not its id"
    );

    let refused = commit(
        &h,
        &owner,
        ops_on(&document, Some(&first), vec![replace(&beta, "x")]),
    )
    .await
    .expect_err("a retired id is refused");
    assert!(
        matches!(refused, DocumentsError::UnknownNode { .. }),
        "{refused}"
    );
}

#[tokio::test]
async fn edits_to_different_blocks_on_a_stale_base_merge() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("merge");
    let (document, first) = memo(&h, &owner).await;
    let base = ids(&h, &owner, &document, &first).await;
    let (second, _) = landed(
        commit(
            &h,
            &owner,
            ops_on(
                &document,
                Some(&first),
                vec![replace(block(&base, "alpha"), "ALPHA")],
            ),
        )
        .await
        .expect("first edit"),
    );
    let outcome = commit(
        &h,
        &owner,
        ops_on(
            &document,
            Some(&first),
            vec![replace(block(&base, "gamma"), "GAMMA")],
        ),
    )
    .await
    .expect("second edit on the stale base");
    let CommitOutcome::Merged(landed) = outcome else {
        panic!("expected Merged, got {outcome:?}");
    };
    let (revision, merged_from) = (landed.revision, landed.ack.merged_from);
    assert_eq!(merged_from, Some(second));
    let head = ids(&h, &owner, &document, &revision.id).await;
    let shown = texts(&head);
    assert!(shown.contains(&"ALPHA".to_owned()), "{shown:?}");
    assert!(shown.contains(&"GAMMA".to_owned()), "{shown:?}");
}

#[tokio::test]
async fn the_same_block_on_a_stale_base_returns_its_current_text() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("overlap");
    let (document, first) = memo(&h, &owner).await;
    let base = ids(&h, &owner, &document, &first).await;
    let alpha = block(&base, "alpha");
    commit(
        &h,
        &owner,
        ops_on(&document, Some(&first), vec![replace(alpha, "theirs")]),
    )
    .await
    .expect("their edit");
    let refused = commit(
        &h,
        &owner,
        ops_on(&document, Some(&first), vec![replace(alpha, "mine")]),
    )
    .await
    .expect_err("an edit to a changed block is refused");
    let DocumentsError::StaleAnchor {
        node, current_text, ..
    } = &refused
    else {
        panic!("expected StaleAnchor, got {refused}");
    };
    assert_eq!(node, &alpha.id);
    assert_eq!(current_text, "theirs");
    assert!(refused.to_string().contains("never resend"), "{refused}");
}

async fn bytes_at(
    h: &Harness,
    owner: &AuthenticatedUser,
    document: &DocumentId,
    revision: &DocumentRevisionId,
) -> Vec<u8> {
    h.service
        .read_bytes(owner, document, Some(revision), &trace())
        .await
        .expect("read bytes")
        .1
}

fn upload_over(base: &[u8], ops: &[EditOp]) -> Vec<u8> {
    let paths = DocxCodec.project(base).expect("project upload base");
    DocxCodec.apply(base, &paths, ops).expect("edit upload")
}

fn path_op(node: &ProjectionNode, text: &str) -> EditOp {
    EditOp::ReplaceText {
        node: NodeTarget::Id(DocumentNodeId::new(
            node.attrs.get("path").expect("path attr").clone(),
        )),
        expected_hash: Some(node.hash8.clone()),
        text: text.to_owned(),
        content_as: ContentAs::Text,
    }
}

fn upload(document: &DocumentId, base: &DocumentRevisionId, bytes: Vec<u8>) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: Some(base.clone()),
        body: CommitBody::Bytes(bytes),
        author: Author {
            actor_kind: ActorKind::Bridge,
            origin: RevisionOrigin::Api,
            attribution: RevisionAttribution::default(),
        },
        message: None,
        lock_holder_id: None,
        comments: CommentPolicy::default(),
        op_id: None,
    }
}

#[tokio::test]
async fn a_stale_office_upload_merges_onto_an_agent_edit() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("upload");
    let (document, first) = memo(&h, &owner).await;
    let base = ids(&h, &owner, &document, &first).await;
    let base_bytes = bytes_at(&h, &owner, &document, &first).await;
    commit(
        &h,
        &owner,
        ops_on(
            &document,
            Some(&first),
            vec![replace(block(&base, "alpha"), "by agent")],
        ),
    )
    .await
    .expect("agent edit");
    let uploaded = upload_over(&base_bytes, &[path_op(block(&base, "gamma"), "by person")]);
    let outcome = commit(&h, &owner, upload(&document, &first, uploaded))
        .await
        .expect("stale upload");
    let (revision, merged) = landed(outcome);
    assert!(merged);
    let head = ids(&h, &owner, &document, &revision).await;
    let shown = texts(&head);
    assert!(shown.contains(&"by agent".to_owned()), "{shown:?}");
    assert!(shown.contains(&"by person".to_owned()), "{shown:?}");
    assert_eq!(block(&head, "by person").id, block(&base, "gamma").id);
}

#[tokio::test]
async fn a_stale_upload_the_diff_cannot_express_conflicts() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("table");
    let (document, first) = memo(&h, &owner).await;
    let base = ids(&h, &owner, &document, &first).await;
    let base_bytes = bytes_at(&h, &owner, &document, &first).await;
    commit(
        &h,
        &owner,
        ops_on(
            &document,
            Some(&first),
            vec![replace(block(&base, "alpha"), "agent")],
        ),
    )
    .await
    .expect("agent edit");
    let uploaded = upload_over(
        &base_bytes,
        &[EditOp::AppendMarkdown {
            markdown: "| a | b |\n|---|---|\n| 1 | 2 |".to_owned(),
        }],
    );
    let outcome = commit(&h, &owner, upload(&document, &first, uploaded))
        .await
        .expect("stale upload");
    assert!(
        matches!(outcome, CommitOutcome::Conflict { .. }),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn restore_and_insert_from_revision_bring_back_history() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("restore");
    let (document, first) = memo(&h, &owner).await;
    let base = ids(&h, &owner, &document, &first).await;
    let beta = block(&base, "beta").clone();
    let (second, _) = landed(
        commit(
            &h,
            &owner,
            ops_on(
                &document,
                Some(&first),
                vec![EditOp::Delete {
                    node: NodeTarget::Id(beta.id.clone()),
                    expected_hash: Some(beta.hash8.clone()),
                }],
            ),
        )
        .await
        .expect("delete beta"),
    );
    let copy_back = EditOp::InsertFromRevision {
        revision_id: first.clone(),
        node: beta.id.clone(),
        after: NodeTarget::Id(block(&base, "alpha").id.clone()),
    };
    let (third, _) = landed(
        commit(
            &h,
            &owner,
            ops_on(&document, Some(&second), vec![copy_back]),
        )
        .await
        .expect("insert from revision"),
    );
    let restored_text = texts(&ids(&h, &owner, &document, &third).await);
    assert_eq!(restored_text, vec!["alpha", "beta", "gamma"]);

    let restore = || EditOp::Restore {
        revision_id: second.clone(),
    };
    let stale = commit(&h, &owner, ops_on(&document, Some(&first), vec![restore()]))
        .await
        .expect("stale restore");
    assert!(matches!(stale, CommitOutcome::Conflict { .. }), "{stale:?}");
    let (fourth, _) = landed(
        commit(&h, &owner, ops_on(&document, Some(&third), vec![restore()]))
            .await
            .expect("restore at head"),
    );
    let at_second = bytes_at(&h, &owner, &document, &second).await;
    assert_eq!(bytes_at(&h, &owner, &document, &fourth).await, at_second);
}

#[tokio::test]
async fn revisions_carry_attribution_into_history_and_the_report_view() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("attributed");
    let (document, first) = memo(&h, &owner).await;
    let attribution = RevisionAttribution {
        session_id: Some(SessionId::new(format!("sess_{}", uuid::Uuid::new_v4()))),
        context_id: Some(ContextId::generate()),
        trace_id: Some(TraceId::generate()),
        task_id: Some(TaskId::new(uuid::Uuid::new_v4().to_string())),
        on_behalf_of: Some(owner.id.clone()),
    };
    let base = ids(&h, &owner, &document, &first).await;
    let mut request = ops_on(
        &document,
        Some(&first),
        vec![replace(block(&base, "beta"), "b")],
    );
    request.author = Author {
        actor_kind: ActorKind::Agent,
        origin: RevisionOrigin::Api,
        attribution: attribution.clone(),
    };
    let (revision, _) = landed(commit(&h, &owner, request).await.expect("attributed edit"));
    let history = h
        .service
        .history(
            &owner,
            &document,
            HistoryPage {
                limit: 5,
                before: None,
            },
            &trace(),
        )
        .await
        .expect("history");
    let recorded = history
        .iter()
        .find(|r| r.id == revision)
        .expect("revision in history");
    assert_eq!(recorded.attribution, attribution);

    let pool = test_db_pool().await.pool();
    let row: (Option<String>, Option<String>, Option<i64>) = sqlx::query_as(
        "SELECT context_id, on_behalf_of, size_bytes FROM report_document_revisions \
         WHERE id = $1::uuid",
    )
    .bind(revision.as_str())
    .fetch_one(&*pool)
    .await
    .expect("report row");
    assert_eq!(
        row.0.as_deref(),
        attribution.context_id.as_ref().map(ContextId::as_str)
    );
    assert_eq!(row.1.as_deref(), Some(owner.id.as_str()));
    assert!(row.2.is_some_and(|size| size > 0));
}
