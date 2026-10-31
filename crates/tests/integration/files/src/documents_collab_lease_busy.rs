use std::time::Duration;

use systemprompt_files::documents::collab::{Checkpoint, JoinMode, Joined};
use systemprompt_files::documents::repository::{CheckpointClaim, CheckpointLease};
use systemprompt_identifiers::DocumentId;
use systemprompt_models::auth::AuthenticatedUser;

use crate::documents_collab::{Collab, client, collab, edit, html_document, request};
use crate::documents_service_fixture::{person, trace};

async fn edited(c: &Collab, owner: &AuthenticatedUser) -> (DocumentId, Joined) {
    let document = html_document(c, owner).await;
    let joined = c
        .collab
        .join(owner, &document, JoinMode::Edit, &trace())
        .await
        .expect("join");
    let doc = client(&joined);
    let update = edit(&doc, "<p>Held</p>");
    c.collab
        .apply(owner, request(&document, &joined, &doc, update), &trace())
        .await
        .expect("apply");
    (document, joined)
}

async fn hold_lease(c: &Collab, joined: &Joined) -> CheckpointLease {
    let lease = CheckpointLease::generate();
    let claim =
        c.h.repos
            .edit_sessions
            .claim_checkpoint(&joined.session_id, &lease, Duration::from_secs(60))
            .await
            .expect("claim");
    assert!(matches!(claim, CheckpointClaim::Claimed(_)), "{claim:?}");
    lease
}

#[tokio::test]
async fn leaving_while_another_replica_holds_the_lease_keeps_the_session_open() {
    let c = collab().await;
    let owner = person("lease-busy-leave");
    let (document, joined) = edited(&c, &owner).await;
    let _held = hold_lease(&c, &joined).await;

    let left = c
        .collab
        .leave(&owner, &document, &joined.session_id, &trace())
        .await
        .expect("leave");

    assert!(!left.closed);
    assert_eq!(left.checkpoint, Checkpoint::Unchanged);
    let open =
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .expect("the session stays open");
    assert_eq!(open.id, joined.session_id);
    assert!(open.checkpoint_revision_id.is_none());
}

#[tokio::test]
async fn trashing_while_another_replica_holds_the_lease_still_closes_the_session() {
    let c = collab().await;
    let owner = person("lease-busy-trash");
    let (document, joined) = edited(&c, &owner).await;
    let _held = hold_lease(&c, &joined).await;
    let found =
        c.h.repos
            .documents
            .find(&document)
            .await
            .expect("find")
            .expect("document");

    let checkpoint = c
        .collab
        .close_for_trash(&owner, &found, &trace())
        .await
        .expect("close for trash");

    assert_eq!(checkpoint, Checkpoint::Unchanged);
    assert!(
        c.h.repos
            .edit_sessions
            .find_open(&document)
            .await
            .expect("find")
            .is_none()
    );
}
