use systemprompt_files::UploadLimits;
use systemprompt_files::uploads::{Allowance, NewUpload, RefusalReason, UploadError, UploadStatus};
use systemprompt_identifiers::DocumentUploadId;

use crate::uploads_rig::{body, hash_of, owner, request, rig, rig_with, trace};

const TEXT: &str = "text/plain";

#[tokio::test]
async fn admission_refuses_bad_names_sizes_and_types() {
    let rig = rig().await;
    let user = owner();
    let cases = [
        request("../escape.txt", TEXT, b"x"),
        request("dir\\file.txt", TEXT, b"x"),
        request("   ", TEXT, b"x"),
        NewUpload {
            size: 0,
            ..request("empty.txt", TEXT, b"x")
        },
        request("weird.txt", "not a mime", b"x"),
    ];
    for case in cases {
        let error = rig
            .service
            .create(&user, case, &trace())
            .await
            .expect_err("refused");
        assert!(matches!(error, UploadError::Invalid(_)), "{error:?}");
    }
    let huge = NewUpload {
        size: rig.service.max_upload_bytes() + 1,
        ..request("huge.bin", TEXT, b"x")
    };
    let error = rig
        .service
        .create(&user, huge, &trace())
        .await
        .expect_err("too large");
    assert!(matches!(error, UploadError::TooLarge { .. }), "{error:?}");
}

#[tokio::test]
async fn inline_uploads_are_bounded_and_must_match_their_declared_size() {
    let limits = UploadLimits {
        inline_max_bytes: 4,
        ..UploadLimits::default()
    };
    let rig = rig_with(limits).await;
    let user = owner();
    let error = rig
        .service
        .put_inline(
            &user,
            request("a.txt", TEXT, b"hello"),
            b"hello".to_vec(),
            &trace(),
        )
        .await
        .expect_err("over inline cap");
    assert!(
        matches!(error, UploadError::TooLarge { size: 5, limit: 4 }),
        "{error:?}"
    );
    let error = rig
        .service
        .put_whole(
            &user,
            request("a.txt", TEXT, b"abcd"),
            b"abc".to_vec(),
            &trace(),
            Allowance::OneShot,
        )
        .await
        .expect_err("size mismatch");
    assert!(matches!(error, UploadError::Invalid(_)), "{error:?}");
}

#[tokio::test]
async fn a_multi_part_session_completes_and_a_second_complete_is_deduplicated() {
    let limits = UploadLimits {
        part_size_bytes: 4,
        ..UploadLimits::default()
    };
    let rig = rig_with(limits).await;
    let user = owner();
    let bytes = b"0123456789";
    let session = rig
        .service
        .create(&user, request("parts.txt", TEXT, bytes), &trace())
        .await
        .expect("create");
    assert_eq!(session.part_count(), 3);
    let missing = rig
        .service
        .complete(&user, &session.id)
        .await
        .expect_err("no parts yet");
    assert!(
        matches!(missing, UploadError::MissingParts { ref missing } if *missing == vec![1, 2, 3])
    );
    for (part, chunk) in (1..).zip(bytes.chunks(4)) {
        rig.service
            .put_part(&user, &session.id, part, body(chunk))
            .await
            .expect("part");
    }
    let progress = rig
        .service
        .progress(&user, &session.id)
        .await
        .expect("progress");
    assert_eq!(progress.received_bytes(), 10);
    assert!(progress.missing_parts().is_empty());
    let first = rig
        .service
        .complete(&user, &session.id)
        .await
        .expect("complete");
    assert_eq!(first.blob, hash_of(bytes));
    assert_eq!(first.checked_kind, "text");
    let again = rig
        .service
        .complete(&user, &session.id)
        .await
        .expect("idempotent");
    assert!(again.deduplicated);
    assert_eq!(again.blob, first.blob);
    let closed = rig
        .service
        .put_part(&user, &session.id, 1, body(b"0123"))
        .await
        .expect_err("closed");
    assert!(
        matches!(
            closed,
            UploadError::Closed {
                status: UploadStatus::Completed
            }
        ),
        "{closed:?}"
    );
}

#[tokio::test]
async fn parts_outside_the_session_are_refused() {
    let rig = rig().await;
    let user = owner();
    let session = rig
        .service
        .create(&user, request("one.txt", TEXT, b"abc"), &trace())
        .await
        .expect("create");
    for part in [0, 2] {
        let error = rig
            .service
            .put_part(&user, &session.id, part, body(b"abc"))
            .await
            .expect_err("range");
        assert!(
            matches!(error, UploadError::PartOutOfRange { parts: 1, .. }),
            "{error:?}"
        );
    }
}

#[tokio::test]
async fn another_owner_cannot_see_or_complete_a_session() {
    let rig = rig().await;
    let user = owner();
    let session = rig
        .service
        .create(&user, request("mine.txt", TEXT, b"abc"), &trace())
        .await
        .expect("create");
    let stranger = owner();
    assert!(matches!(
        rig.service.progress(&stranger, &session.id).await,
        Err(UploadError::NotFound)
    ));
    assert!(matches!(
        rig.service.complete(&stranger, &session.id).await,
        Err(UploadError::NotFound)
    ));
    let unknown = DocumentUploadId::generate();
    assert!(matches!(
        rig.service.complete(&user, &unknown).await,
        Err(UploadError::NotFound)
    ));
}

#[tokio::test]
async fn a_checksum_mismatch_refuses_and_closes_the_session() {
    let rig = rig().await;
    let user = owner();
    let declared = NewUpload {
        sha256: hash_of(b"something else"),
        ..request("lie.txt", TEXT, b"abc")
    };
    let session = rig
        .service
        .create(&user, declared, &trace())
        .await
        .expect("create");
    rig.service
        .put_part(&user, &session.id, 1, body(b"abc"))
        .await
        .expect("part");
    let error = rig
        .service
        .complete(&user, &session.id)
        .await
        .expect_err("mismatch");
    assert!(
        matches!(
            error,
            UploadError::Refused {
                reason: RefusalReason::ChecksumMismatch,
                ..
            }
        ),
        "{error:?}"
    );
    let progress = rig
        .service
        .progress(&user, &session.id)
        .await
        .expect("progress");
    assert_eq!(progress.session.status, UploadStatus::Refused);
    assert!(
        progress
            .session
            .refusal
            .as_deref()
            .is_some_and(|r| r.contains("sha256"))
    );
    let again = rig
        .service
        .complete(&user, &session.id)
        .await
        .expect_err("closed");
    assert!(
        matches!(
            again,
            UploadError::Closed {
                status: UploadStatus::Refused
            }
        ),
        "{again:?}"
    );
}

#[tokio::test]
async fn open_sessions_and_the_daily_quota_are_capped_per_owner() {
    let limits = UploadLimits {
        max_open_sessions: 1,
        daily_quota_bytes: 5,
        ..UploadLimits::default()
    };
    let rig = rig_with(limits).await;
    let user = owner();
    rig.service
        .create(&user, request("a.txt", TEXT, b"abc"), &trace())
        .await
        .expect("first");
    let error = rig
        .service
        .create(&user, request("b.txt", TEXT, b"abc"), &trace())
        .await
        .expect_err("second open");
    assert!(
        matches!(error, UploadError::TooManyOpen { limit: 1, .. }),
        "{error:?}"
    );
    let error = rig
        .service
        .create(&owner(), request("big.txt", TEXT, b"abcdefgh"), &trace())
        .await
        .expect_err("quota");
    assert!(
        matches!(error, UploadError::QuotaExceeded { limit: 5, .. }),
        "{error:?}"
    );
}

#[tokio::test]
async fn the_mime_is_normalised_before_it_is_stored() {
    let rig = rig().await;
    let user = owner();
    let session = rig
        .service
        .create(&user, request("n.txt", "  Text/Plain ", b"abc"), &trace())
        .await
        .expect("create");
    assert_eq!(session.declared_mime, "text/plain");
    assert_eq!(
        rig.service.limits().part_size_bytes,
        UploadLimits::default().part_size_bytes
    );
}
