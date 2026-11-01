use systemprompt_files::UploadLimits;
use systemprompt_files::uploads::{
    DocumentUploadRepository, UploadError, UploadSession, UploadStatus,
};
use systemprompt_identifiers::{DocumentUploadId, UserId};
use systemprompt_test_fixtures::test_db_pool;

use crate::uploads_rig::{UploadRig, body, hash_of, request, rig_with, trace};

const TEXT: &str = "text/plain";

fn four_byte_parts() -> UploadLimits {
    UploadLimits {
        part_size_bytes: 4,
        ..UploadLimits::default()
    }
}

fn uploader() -> UserId {
    UserId::new("wp9-lifecycle-uploader")
}

async fn repo() -> DocumentUploadRepository {
    DocumentUploadRepository::new(&test_db_pool().await)
}

async fn open(rig: &UploadRig, bytes: &[u8]) -> UploadSession {
    rig.service
        .create(&uploader(), request("parts.txt", TEXT, bytes), &trace())
        .await
        .expect("create")
}

async fn backdate(id: &DocumentUploadId) {
    let pool = test_db_pool().await.pool();
    sqlx::query(
        "UPDATE document_uploads SET expires_at = now() - interval '1 hour' WHERE id = $1::uuid",
    )
    .bind(id.as_str())
    .execute(&*pool)
    .await
    .expect("backdate");
}

async fn status_of(id: &DocumentUploadId) -> UploadStatus {
    repo()
        .await
        .find(id)
        .await
        .expect("find")
        .expect("row")
        .status
}

#[tokio::test]
async fn a_claim_is_exclusive_and_a_release_reopens_the_session() {
    let rig = rig_with(four_byte_parts()).await;
    let session = open(&rig, b"abcd").await;
    let uploads = repo().await;

    assert!(uploads.claim(&session.id).await.expect("claim"));
    assert!(!uploads.claim(&session.id).await.expect("second claim"));
    assert_eq!(status_of(&session.id).await, UploadStatus::Completing);
    uploads.touch_claim(&session.id).await.expect("touch");

    assert!(uploads.release(&session.id).await.expect("release"));
    assert!(!uploads.release(&session.id).await.expect("second release"));
    assert_eq!(status_of(&session.id).await, UploadStatus::Open);

    assert!(
        uploads
            .mark_closed(&session.id, UploadStatus::Open, UploadStatus::Aborted, None)
            .await
            .expect("close")
    );
    assert!(
        !uploads
            .mark_closed(&session.id, UploadStatus::Open, UploadStatus::Aborted, None)
            .await
            .expect("close again")
    );
    assert!(!uploads.claim(&session.id).await.expect("claim closed"));
}

#[tokio::test]
async fn completing_a_session_another_caller_claimed_reports_it_closed() {
    let rig = rig_with(four_byte_parts()).await;
    let session = open(&rig, b"abcd").await;
    rig.service
        .put_part(&uploader(), &session.id, 1, body(b"abcd"))
        .await
        .expect("part");
    assert!(repo().await.claim(&session.id).await.expect("claim"));

    let error = rig
        .service
        .complete(&uploader(), &session.id)
        .await
        .expect_err("claimed elsewhere");
    assert!(
        matches!(
            error,
            UploadError::Closed {
                status: UploadStatus::Completing
            }
        ),
        "{error:?}"
    );
}

#[tokio::test]
async fn an_expired_session_cannot_be_claimed_and_expiry_closes_it() {
    let rig = rig_with(four_byte_parts()).await;
    let session = open(&rig, b"abcdefgh").await;
    rig.service
        .put_part(&uploader(), &session.id, 1, body(b"abcd"))
        .await
        .expect("part");
    backdate(&session.id).await;
    let uploads = repo().await;

    assert!(!uploads.claim(&session.id).await.expect("claim expired"));
    let listed = uploads.list_expired(10_000).await.expect("list");
    assert!(listed.contains(&session.id));

    let outcome = rig.service.expire(10_000).await.expect("expire");
    assert!(outcome.expired >= 1);
    assert_eq!(status_of(&session.id).await, UploadStatus::Expired);
    let listed = uploads.list_expired(10_000).await.expect("list again");
    assert!(!listed.contains(&session.id));
}

#[tokio::test]
async fn a_short_part_is_refused_and_its_bytes_are_refunded() {
    let rig = rig_with(four_byte_parts()).await;
    let session = open(&rig, b"abcdefgh").await;

    let short = rig
        .service
        .put_part(&uploader(), &session.id, 1, body(b"ab"))
        .await
        .expect_err("short part");
    assert!(
        matches!(
            short,
            UploadError::PartSize {
                part: 1,
                expected: 4,
                actual: 2
            }
        ),
        "{short:?}"
    );
    let long = rig
        .service
        .put_part(&uploader(), &session.id, 2, body(b"efghij"))
        .await
        .expect_err("long part");
    assert!(
        matches!(
            long,
            UploadError::PartSize {
                part: 2,
                expected: 4,
                ..
            }
        ),
        "{long:?}"
    );

    let progress = rig
        .service
        .progress(&uploader(), &session.id)
        .await
        .expect("progress");
    assert_eq!(progress.received_bytes(), 0);
    assert_eq!(progress.missing_parts(), vec![1, 2]);
}

#[tokio::test]
async fn a_resent_part_replaces_the_first_attempt() {
    let rig = rig_with(four_byte_parts()).await;
    let bytes = b"abcdefgh";
    let session = open(&rig, bytes).await;
    rig.service
        .put_part(&uploader(), &session.id, 1, body(b"zzzz"))
        .await
        .expect("first attempt");
    rig.service
        .put_part(&uploader(), &session.id, 1, body(b"abcd"))
        .await
        .expect("second attempt");
    rig.service
        .put_part(&uploader(), &session.id, 2, body(b"efgh"))
        .await
        .expect("part two");

    let progress = rig
        .service
        .progress(&uploader(), &session.id)
        .await
        .expect("progress");
    assert_eq!(progress.received_bytes(), 8);

    let done = rig
        .service
        .complete(&uploader(), &session.id)
        .await
        .expect("complete");
    assert_eq!(done.blob, hash_of(bytes));
    assert_eq!(done.size_bytes, 8);
    assert_eq!(status_of(&session.id).await, UploadStatus::Completed);
}
