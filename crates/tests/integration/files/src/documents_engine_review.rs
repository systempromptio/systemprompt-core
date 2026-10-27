//! Proposal policy and review under the revision engine (an agent's `land`
//! under a `propose` folder, self-review, the unlanded-accept sweep) and
//! the Office paths: a stale upload that changes formatting, and
//! `insert_from_revision` copying a table natively.

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    CommentPolicy, CommitOutcome, CommitRequest, EditModeRequest, ReviewRequest,
};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::{
    AccessLevel, ActorKind, AgentEditPolicy, ContentAs, EditMode, EditOp, NodeKind, NodeTarget,
    Projection, ProposalStatus,
};
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_proposals::{folder, grant, memo, outline, propose, rewrite, texts};
use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, create, harness, person, trace,
};

fn ops(
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
        comments: CommentPolicy::detach(),
        op_id: None,
    }
}

async fn land(h: &Harness, who: &AuthenticatedUser, request: CommitRequest) -> DocumentRevisionId {
    match h
        .service
        .commit(who, request, &trace())
        .await
        .expect("commit")
    {
        CommitOutcome::Committed(landed) | CommitOutcome::Merged(landed) => landed.revision.id,
        CommitOutcome::Conflict { head, .. } => panic!("conflict at {}", head.id),
    }
}

#[tokio::test]
async fn an_agent_land_under_a_propose_folder_is_refused() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("agent-land");
    let parent = folder(&h, &owner, None).await;
    h.service
        .set_folder_agent_edit_policy(&owner, &parent, AgentEditPolicy::Propose, &trace())
        .await
        .expect("policy");
    let (document, _) = memo(&h, &owner, Some(&parent)).await;
    let refused = h
        .service
        .edit_mode(
            &owner,
            EditModeRequest {
                document: &document,
                requested: Some(EditMode::Land),
                actor: ActorKind::Agent,
            },
            &trace(),
        )
        .await;
    match refused {
        Err(DocumentsError::Forbidden { policy, .. }) => {
            assert_eq!(policy.as_deref(), Some("propose"));
        },
        other => panic!("expected forbidden, got {other:?}"),
    }
    let user = h
        .service
        .edit_mode(
            &owner,
            EditModeRequest {
                document: &document,
                requested: Some(EditMode::Land),
                actor: ActorKind::User,
            },
            &trace(),
        )
        .await
        .expect("a user lands");
    assert_eq!(user, EditMode::Land);
}

#[tokio::test]
async fn a_proposer_cannot_accept_their_own_proposal() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("self-owner");
    let editor = person("self-editor");
    let (document, _) = memo(&h, &owner, None).await;
    grant(&h, &owner, &document, &editor, AccessLevel::Edit).await;
    let head = outline(&h, &owner, &document, None).await;
    let proposed = propose(&h, &editor, &document, vec![rewrite(&head, "beta", "BETA")])
        .await
        .expect("propose");
    let review = || ReviewRequest {
        proposal_id: proposed.proposal.id.clone(),
        note: None,
        reviewer: api_author(),
    };
    let own = h.service.accept_proposal(&editor, review(), &trace()).await;
    assert!(
        matches!(own, Err(DocumentsError::SelfReview { .. })),
        "{own:?}"
    );
    let accepted = h
        .service
        .accept_proposal(&owner, review(), &trace())
        .await
        .expect("owner accepts");
    assert_eq!(
        accepted.proposal.landed_revision_id.as_ref(),
        Some(&accepted.revision.id)
    );
    assert!(accepted.revision.landed_at.is_some());
}

#[tokio::test]
async fn an_accept_that_never_landed_is_reopened_by_the_sweep() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("sweep-owner");
    let proposer = person("sweep-proposer");
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
    let pool = test_db_pool().await.pool();
    sqlx::query(
        "UPDATE document_proposals
         SET status = 'accepted', updated_at = now() - interval '10 minutes'
         WHERE id = $1::uuid",
    )
    .bind(proposed.proposal.id.as_str())
    .execute(&*pool)
    .await
    .expect("half-accept");
    let reopened = h
        .repos
        .proposals
        .reopen_unlanded(5, 10_000)
        .await
        .expect("sweep");
    assert!(reopened >= 1);
    let proposal = h
        .repos
        .proposals
        .find(&proposed.proposal.id)
        .await
        .expect("find")
        .expect("proposal");
    assert_eq!(proposal.status, ProposalStatus::Open);
}

async fn docx(
    h: &Harness,
    owner: &AuthenticatedUser,
    markdown: &str,
) -> (DocumentId, DocumentRevisionId) {
    let mut request = create("blank-docx", None);
    request.ops = vec![EditOp::AppendMarkdown {
        markdown: markdown.to_owned(),
    }];
    let (document, revision) = h
        .service
        .create_from_template(owner, request, &trace())
        .await
        .expect("create docx");
    (document.id, revision.id)
}

fn block<'p>(
    projection: &'p Projection,
    text: &str,
) -> &'p systemprompt_models::documents::ProjectionNode {
    projection
        .nodes()
        .find(|n| n.text.as_deref() == Some(text))
        .unwrap_or_else(|| panic!("no block {text}"))
}

fn replace(projection: &Projection, from: &str, to: &str, content_as: ContentAs) -> EditOp {
    let node = block(projection, from);
    EditOp::ReplaceText {
        node: NodeTarget::Id(node.id.clone()),
        expected_hash: Some(node.hash8.clone()),
        text: to.to_owned(),
        content_as,
    }
}

async fn stale_upload(
    h: &Harness,
    owner: &AuthenticatedUser,
    to: &str,
    content_as: ContentAs,
) -> CommitOutcome {
    let (document, rev0) = docx(h, owner, "alpha one two\n\nbeta one two\n\ngamma one two").await;
    let base = outline(h, owner, &document, None).await;
    let edited = land(
        h,
        owner,
        ops(
            &document,
            None,
            vec![replace(&base, "beta one two", to, content_as)],
        ),
    )
    .await;
    let (_, upload) = h
        .service
        .read_bytes(owner, &document, Some(&edited), &trace())
        .await
        .expect("upload bytes");
    let restored = land(
        h,
        owner,
        ops(
            &document,
            Some(&edited),
            vec![EditOp::Restore {
                revision_id: rev0.clone(),
            }],
        ),
    )
    .await;
    let head = outline(h, owner, &document, None).await;
    land(
        h,
        owner,
        ops(
            &document,
            Some(&restored),
            vec![replace(
                &head,
                "alpha one two",
                "alpha one two three",
                ContentAs::Text,
            )],
        ),
    )
    .await;
    let request = CommitRequest {
        body: CommitBody::Bytes(upload),
        ..ops(&document, Some(&rev0), Vec::new())
    };
    h.service
        .commit(owner, request, &trace())
        .await
        .expect("upload")
}

#[tokio::test]
async fn a_stale_upload_that_only_changes_text_merges() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("upload-text");
    let outcome = stale_upload(&h, &owner, "beta one two three", ContentAs::Text).await;
    assert!(matches!(outcome, CommitOutcome::Merged(_)), "{outcome:?}");
}

#[tokio::test]
async fn a_stale_upload_that_also_changes_formatting_conflicts() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("upload-format");
    let outcome = stale_upload(&h, &owner, "**beta one two three**", ContentAs::Markdown).await;
    assert!(
        matches!(outcome, CommitOutcome::Conflict { .. }),
        "{outcome:?}"
    );
}

#[tokio::test]
async fn insert_from_revision_copies_a_docx_table() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("copy-table");
    let (document, with_table) = docx(
        &h,
        &owner,
        "intro\n\n| a | b |\n|---|---|\n| 1 | 2 |\n\noutro",
    )
    .await;
    let head = outline(&h, &owner, &document, None).await;
    let table = head
        .nodes()
        .find(|n| n.kind == NodeKind::Table)
        .expect("table")
        .clone();
    let removed = land(
        &h,
        &owner,
        ops(
            &document,
            None,
            vec![EditOp::Delete {
                node: NodeTarget::Id(table.id.clone()),
                expected_hash: Some(table.hash8.clone()),
            }],
        ),
    )
    .await;
    let without = outline(&h, &owner, &document, Some(&removed)).await;
    assert!(without.nodes().all(|n| n.kind != NodeKind::Table));
    let intro = block(&without, "intro").id.clone();
    let copied = land(
        &h,
        &owner,
        ops(
            &document,
            None,
            vec![EditOp::InsertFromRevision {
                revision_id: with_table,
                node: table.id,
                after: NodeTarget::Id(intro),
            }],
        ),
    )
    .await;
    let after = outline(&h, &owner, &document, Some(&copied)).await;
    assert!(after.nodes().any(|n| n.kind == NodeKind::Table));
    let shown = texts(&after);
    for cell in ["a", "b", "1", "2"] {
        assert!(
            shown.iter().any(|t| t == cell),
            "{cell} missing from {shown:?}"
        );
    }
}
