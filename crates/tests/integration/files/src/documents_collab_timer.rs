use std::sync::Arc;
use std::time::Duration;

use systemprompt_files::documents::collab::{CollabService, CollabTuning, JoinMode};
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{CommentPolicy, CommitOutcome, CommitRequest};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::RevisionOriginKind;

use crate::documents_collab::{Collab, client, collab, edit, html_document, request};
use crate::documents_service_fixture::{api_author, person, trace};

fn eager(c: &Collab) -> Arc<CollabService> {
    Arc::new(CollabService::new(
        Arc::clone(&c.documents),
        CollabTuning {
            snapshot_after: Duration::ZERO,
            snapshot_after_updates: 10_000,
            checkpoint_after: Duration::ZERO,
            tick: Duration::from_millis(10),
            evict_after: Duration::ZERO,
        },
    ))
}

async fn head_html(c: &Collab, owner: &AuthenticatedUser, document: &DocumentId) -> String {
    let (_, bytes) = c
        .documents
        .read_bytes(owner, document, None, &trace())
        .await
        .expect("read");
    String::from_utf8(bytes).expect("utf8")
}

#[tokio::test]
async fn a_tick_snapshots_checkpoints_and_evicts_due_sessions() {
    let c = collab().await;
    let service = eager(&c);
    let owner = person("collab-tick");
    let document = html_document(&c, &owner).await;
    let joined = service
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>Ticked</p>");
    service
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");

    service.tick().await;

    let session =
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .expect("session stays open");
    assert!(session.snapshot_sha256.is_some(), "snapshot stored");
    let checkpoint = session.checkpoint_revision_id.expect("checkpoint recorded");
    let revision =
        c.h.repos
            .revisions
            .find(&checkpoint)
            .await
            .expect("find")
            .expect("revision");
    assert_eq!(revision.origin_kind, RevisionOriginKind::EditSession);
    assert!(
        head_html(&c, &owner, &document)
            .await
            .contains("<p>Ticked</p>")
    );

    service.tick().await;
    let again =
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .expect("session");
    assert_eq!(again.checkpoint_revision_id, Some(checkpoint));
}

#[tokio::test]
async fn a_checkpoint_conflicting_with_an_outside_write_leaves_the_head_alone() {
    let c = collab().await;
    let service = eager(&c);
    let owner = person("collab-tick-merge");
    let document = html_document(&c, &owner).await;
    let joined = service
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>Live</p>");
    service
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");

    let (head, bytes) = c
        .documents
        .read_bytes(&owner, &document, None, &trace())
        .await
        .expect("read");
    let outside =
        String::from_utf8(bytes)
            .expect("utf8")
            .replacen("<body>", "<body><h1>Outside</h1>", 1);
    let outcome = c
        .documents
        .commit(
            &owner,
            CommitRequest {
                document_id: document.clone(),
                base_revision: Some(head.id),
                body: CommitBody::Bytes(outside.into_bytes()),
                author: api_author(),
                message: None,
                lock_holder_id: None,
                comments: CommentPolicy::default(),
                op_id: None,
            },
            &trace(),
        )
        .await
        .expect("outside commit");
    assert!(
        matches!(outcome, CommitOutcome::Committed { .. }),
        "{outcome:?}"
    );

    service.tick().await;

    let html = head_html(&c, &owner, &document).await;
    assert!(html.contains("<h1>Outside</h1>"), "{html}");
    assert!(!html.contains("<p>Live</p>"), "{html}");
    let session =
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .expect("session stays open");
    assert!(session.checkpoint_revision_id.is_none());
}

#[tokio::test]
async fn the_started_timer_runs_until_cancelled() {
    let c = collab().await;
    let service = eager(&c);
    let owner = person("collab-timer");
    let document = html_document(&c, &owner).await;
    let joined = service
        .join(&owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>Background</p>");
    service
        .apply(&owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");
    let tasks = systemprompt_traits::BackgroundTasks::new();
    service.start(&tasks);
    let mut landed = false;
    for _ in 0..100 {
        if head_html(&c, &owner, &document)
            .await
            .contains("<p>Background</p>")
        {
            landed = true;
            break;
        }
        tokio::time::sleep(Duration::from_millis(20)).await;
    }
    let drained = tasks.shutdown(Duration::from_secs(5)).await;
    assert!(matches!(
        drained,
        systemprompt_traits::DrainOutcome::Drained
    ));
    assert!(landed, "the background tick checkpoints the session");
}
