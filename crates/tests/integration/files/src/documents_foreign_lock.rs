//! A lock held by another holder refuses a commit at the service and at the
//! head compare-and-swap it runs inside the landing transaction.

use std::time::Duration;

use systemprompt_files::documents::DocumentsError;
use systemprompt_files::documents::model::Media;
use systemprompt_files::documents::models::{LockHolder, LockOutcome, LockRequest};
use systemprompt_files::documents::repository::{DocumentRepository, HeadMove};
use systemprompt_files::documents::revision::CommitBody;
use systemprompt_files::documents::service::{
    AcquireLockRequest, CommentPolicy, CommitOutcome, CommitRequest,
};
use systemprompt_identifiers::{DocumentId, DocumentRevisionId, DriveDeviceId};
use systemprompt_models::auth::AuthenticatedUser;
use systemprompt_models::documents::EditOp;

use crate::documents_proposals::{memo, outline, rewrite};
use crate::documents_service_fixture::{
    GroupSource, Harness, allow, api_author, harness, person, trace,
};

const DEVICE_A: &str = "device-a";
const DEVICE_B: &str = "device-b";

fn device(id: &str) -> LockHolder {
    LockHolder::Bridge(DriveDeviceId::new(id))
}

fn ops(document: &DocumentId, ops: Vec<EditOp>, holder: Option<LockHolder>) -> CommitRequest {
    CommitRequest {
        document_id: document.clone(),
        base_revision: None,
        body: CommitBody::Ops(ops),
        author: api_author(),
        message: None,
        lock_holder: holder,
        comments: CommentPolicy::default(),
        op_id: None,
        media: Media::new(),
    }
}

async fn lock(h: &Harness, who: &AuthenticatedUser, document: &DocumentId, holder: &str) {
    let outcome = h
        .service
        .acquire_lock(
            who,
            document,
            AcquireLockRequest {
                holder: device(holder),
                ttl: Duration::from_secs(60),
            },
            &trace(),
        )
        .await
        .expect("acquire");
    assert!(matches!(outcome, LockOutcome::Acquired(_)), "{outcome:?}");
}

async fn head_id(h: &Harness, document: &DocumentId) -> DocumentRevisionId {
    h.repos
        .revisions
        .head(document)
        .await
        .expect("head")
        .expect("a head")
        .id
}

#[tokio::test]
async fn an_ops_commit_under_a_foreign_lock_is_refused_as_lock_held_and_the_head_stays() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("flock-owner");
    let (document, first) = memo(&h, &owner, None).await;
    lock(&h, &owner, &document, DEVICE_A).await;
    let projection = outline(&h, &owner, &document, None).await;

    let anonymous = ops(
        &document,
        vec![rewrite(&projection, "alpha", "ALPHA")],
        None,
    );
    let refused = h
        .service
        .commit(&owner, anonymous, &trace())
        .await
        .expect_err("no holder id under a lock");
    assert!(
        matches!(
            &refused,
            DocumentsError::LockHeld { document_id, holder }
                if *document_id == document && *holder == device(DEVICE_A)
        ),
        "{refused:?}"
    );

    let other = ops(
        &document,
        vec![rewrite(&projection, "beta", "BETA")],
        Some(device(DEVICE_B)),
    );
    let refused = h
        .service
        .commit(&owner, other, &trace())
        .await
        .expect_err("another holder under a lock");
    assert!(
        matches!(refused, DocumentsError::LockHeld { .. }),
        "{refused:?}"
    );
    assert_eq!(head_id(&h, &document).await, first);

    let holder = ops(
        &document,
        vec![rewrite(&projection, "gamma", "GAMMA")],
        Some(device(DEVICE_A)),
    );
    let landed = h
        .service
        .commit(&owner, holder, &trace())
        .await
        .expect("the holder commits");
    assert!(matches!(landed, CommitOutcome::Committed(_)), "{landed:?}");
    assert_ne!(head_id(&h, &document).await, first);
}

#[tokio::test]
async fn the_head_swap_refuses_a_foreign_lock_and_ignores_an_expired_one() {
    let h = harness(allow(), GroupSource::new(&[])).await;
    let owner = person("cas-owner");
    let (document, first) = memo(&h, &owner, None).await;
    let projection = outline(&h, &owner, &document, None).await;
    let second = h
        .service
        .commit(
            &owner,
            ops(
                &document,
                vec![rewrite(&projection, "alpha", "ALPHA")],
                None,
            ),
            &trace(),
        )
        .await
        .expect("second revision")
        .landed()
        .expect("landed")
        .revision
        .id
        .clone();
    assert_eq!(head_id(&h, &document).await, second);

    lock(&h, &owner, &document, DEVICE_A).await;
    let (holder_a, holder_b) = (device(DEVICE_A), device(DEVICE_B));
    let back_to_first = |holder| HeadMove {
        document: &document,
        expected: Some(&second),
        new: &first,
        lock_holder: holder,
    };

    let mut tx = h.repos.documents.begin().await.expect("begin");
    let moved = DocumentRepository::move_head(&mut tx, back_to_first(None))
        .await
        .expect("swap without a holder");
    assert!(!moved);
    let moved = DocumentRepository::move_head(&mut tx, back_to_first(Some(&holder_b)))
        .await
        .expect("swap by another holder");
    assert!(!moved);
    let moved = DocumentRepository::move_head(&mut tx, back_to_first(Some(&holder_a)))
        .await
        .expect("swap by the holder");
    assert!(moved);
    tx.rollback().await.expect("rollback");
    assert_eq!(head_id(&h, &document).await, second);

    let expired = h
        .repos
        .locks
        .acquire(&LockRequest {
            document_id: document.clone(),
            holder_user_id: owner.id.clone(),
            holder: device(DEVICE_A),
            ttl: Duration::ZERO,
        })
        .await
        .expect("renew with no lease");
    assert!(matches!(expired, LockOutcome::Acquired(_)), "{expired:?}");
    assert!(
        h.repos
            .locks
            .current(&document)
            .await
            .expect("current")
            .is_none()
    );
    let mut tx = h.repos.documents.begin().await.expect("begin");
    let moved = DocumentRepository::move_head(&mut tx, back_to_first(Some(&holder_b)))
        .await
        .expect("swap past an expired lock");
    assert!(moved);
    tx.rollback().await.expect("rollback");
}
