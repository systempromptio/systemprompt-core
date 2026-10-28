use std::sync::Arc;
use std::time::Duration;

use loro::{ExportMode, UpdateOptions};
use systemprompt_engine_loro::BODY_CONTAINER;
use systemprompt_files::documents::collab::{
    ApplyRequest, Checkpoint, CollabError, CollabService, CollabTuning, JoinMode,
};
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommentPolicy, CommitOutcome, CommitRequest};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::ProposalStatus;
use systemprompt_test_fixtures::test_db_pool;

use crate::documents_collab::{Collab, client, collab, edit, html_document, loro, request};
use crate::documents_service_fixture::{api_author, person, trace};

async fn head_text(
    c: &Collab,
    owner: &AuthenticatedUser,
    document: &DocumentId,
) -> (DocumentRevisionId, String) {
    let (revision, bytes) = c
        .documents
        .read_bytes(owner, document, None, &trace())
        .await
        .expect("read");
    (revision.id, String::from_utf8(bytes).expect("utf8"))
}

async fn write(c: &Collab, owner: &AuthenticatedUser, document: &DocumentId, from: &str, to: &str) {
    let (head, text) = head_text(c, owner, document).await;
    let outcome = c
        .documents
        .commit(
            owner,
            CommitRequest {
                document_id: document.clone(),
                base_revision: Some(head),
                body: CommitBody::Bytes(text.replacen(from, to, 1).into_bytes()),
                author: api_author(),
                message: None,
                lock_holder_id: None,
                comments: CommentPolicy::default(),
                op_id: None,
            },
            &trace(),
        )
        .await
        .expect("commit");
    assert!(
        matches!(outcome, CommitOutcome::Committed(_)),
        "{outcome:?}"
    );
}

async fn seeded(c: &Collab, owner: &AuthenticatedUser) -> DocumentId {
    let document = html_document(c, owner).await;
    write(c, owner, &document, "</body>", "<p>alpha</p></body>").await;
    document
}

#[tokio::test]
async fn leaving_on_a_conflicting_checkpoint_lands_a_proposal_and_keeps_the_session() {
    let c = collab().await;
    let owner = person("settle-conflict");
    let document = seeded(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    write(&c, &owner, &document, "alpha", "outside").await;
    let doc = client(&joined);
    let before = doc.oplog_vv();
    let text = doc.get_text(BODY_CONTAINER);
    let live = text.to_string().replacen("alpha", "live", 1);
    text.update(&live, UpdateOptions::default())
        .expect("update");
    doc.commit();
    let update = doc.export(ExportMode::updates(&before)).expect("export");
    c.collab
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");
    let left = c
        .collab
        .leave(&owner, &document, &joined.session_id, &trace())
        .await
        .expect("leave");
    assert!(!left.closed);
    let Checkpoint::Conflict { proposal, .. } = left.checkpoint else {
        panic!("expected a conflict, got {:?}", left.checkpoint);
    };
    let proposal = proposal.expect("the live body lands as a proposal");
    let open =
        c.h.repos
            .proposals
            .list(&document, Some(ProposalStatus::Open), 50)
            .await
            .expect("open proposals");
    assert!(open.iter().any(|p| p.id == proposal));
    assert!(
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .is_some()
    );
}

#[tokio::test]
async fn a_viewer_leaving_last_never_checkpoints() {
    let c = collab().await;
    let other = CollabService::new(Arc::clone(&c.documents), CollabTuning::default(), loro());
    let owner = person("settle-owner");
    let document = seeded(&c, &owner).await;
    let (head, _) = head_text(&c, &owner, &document).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>unsaved</p>");
    c.collab
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");
    c.h.repos
        .collab
        .remove_web_presence(&document, &owner.id)
        .await
        .expect("drop editor presence");
    other
        .join(&owner, &document, JoinMode::View, &trace())
        .await
        .expect("view join on a replica that saw no edit");
    let left = other
        .leave(&owner, &document, &joined.session_id, &trace())
        .await
        .expect("viewer leave");
    assert!(!left.closed);
    assert_eq!(left.checkpoint, Checkpoint::Unchanged);
    assert_eq!(head_text(&c, &owner, &document).await.0, head);
    assert!(
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .is_some()
    );
}

#[tokio::test]
async fn sequence_numbers_follow_commit_order_across_replicas() {
    let c = collab().await;
    let other = Arc::new(CollabService::new(
        Arc::clone(&c.documents),
        CollabTuning::default(),
        loro(),
    ));
    let owner = person("settle-seq");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let first = edit(&doc, "<p>one</p>");
    c.collab
        .apply(&owner, request(&document, &joined, &doc, first), &trace())
        .await
        .expect("first");

    let pool = test_db_pool().await;
    let mut held = pool.write_pool().begin().await.expect("begin");
    sqlx::query("SELECT next_seq FROM document_edit_sessions WHERE id = $1::uuid FOR UPDATE")
        .bind(joined.session_id.as_str())
        .execute(&mut *held)
        .await
        .expect("lock session row");
    let second = edit(&doc, "<p>two</p>");
    let blocked = {
        let other = Arc::clone(&other);
        let owner = owner.clone();
        let req: ApplyRequest = request(&document, &joined, &doc, second);
        tokio::spawn(async move { other.apply(&owner, req, &trace()).await })
    };
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert!(
        !blocked.is_finished(),
        "the append waits for the session row"
    );
    held.rollback().await.expect("release");
    let applied = blocked.await.expect("join").expect("second apply");

    let rows =
        c.h.repos
            .collab
            .list_after(&joined.session_id, 0)
            .await
            .expect("rows");
    let seqs: Vec<i64> = rows.iter().map(|row| row.seq).collect();
    assert_eq!(seqs, vec![1, 2]);
    assert_eq!(applied.seq, 2);
    let rejoined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("rejoin first replica");
    assert_eq!(rejoined.seq, 2);
    assert_eq!(
        client(&rejoined).get_text(BODY_CONTAINER).to_string(),
        doc.get_text(BODY_CONTAINER).to_string()
    );
}

#[tokio::test]
async fn a_refused_update_never_reaches_the_log_or_the_cache() {
    let c = collab().await;
    let owner = person("settle-refused");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let refused = c
        .collab
        .apply(
            &owner,
            request(&document, &joined, &doc, b"not a loro update".to_vec()),
            &trace(),
        )
        .await;
    assert!(
        matches!(refused, Err(CollabError::InvalidUpdate(_))),
        "{refused:?}"
    );
    let rows =
        c.h.repos
            .collab
            .list_after(&joined.session_id, 0)
            .await
            .expect("rows");
    assert!(rows.is_empty());
    let again = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("rejoin");
    assert_eq!(again.version, joined.version);
}

#[tokio::test]
async fn trashing_a_document_checkpoints_and_closes_its_session() {
    let c = collab().await;
    c.documents.attach_collab(&c.collab);
    let owner = person("settle-trash");
    let document = html_document(&c, &owner).await;
    let joined = c
        .collab
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>kept</p>");
    c.collab
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");
    let (before, _) = head_text(&c, &owner, &document).await;
    c.documents
        .trash(&owner, &document, &trace())
        .await
        .expect("trash");
    assert!(
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .is_none()
    );
    let latest =
        c.h.repos
            .revisions
            .list(&document, 1, None)
            .await
            .expect("revisions");
    assert_ne!(latest[0].id, before, "the live body is checkpointed");
    assert!(
        c.h.repos
            .presence
            .users_present(&document)
            .await
            .expect("presence")
            .is_empty()
    );
}
