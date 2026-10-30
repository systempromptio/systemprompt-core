use systemprompt_files::UploadLimits;
use systemprompt_files::uploads::{Allowance, NewUpload, UploadError};

use crate::uploads_rig::{owner, request, rig, rig_with, trace};

const TEXT: &str = "text/plain";

#[tokio::test]
async fn admission_refuses_bad_names_sizes_and_types() {
    let rig = rig().await;
    let user = owner();
    let cases = [
        (request("../escape.txt", TEXT, b"x"), "name"),
        (request("dir\\file.txt", TEXT, b"x"), "name"),
        (request("   ", TEXT, b"x"), "name"),
        (
            NewUpload {
                size: 0,
                ..request("empty.txt", TEXT, b"x")
            },
            "empty",
        ),
        (request("weird.txt", "not a mime", b"x"), "mime"),
    ];
    for (case, expected) in cases {
        let error = rig
            .service
            .create(&user, case, &trace())
            .await
            .expect_err("refused");
        let refused = match error {
            UploadError::InvalidName => "name",
            UploadError::EmptyUpload => "empty",
            UploadError::InvalidContentType { .. } => "mime",
            ref other => panic!("unexpected refusal {other:?}"),
        };
        assert_eq!(refused, expected);
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
    assert!(matches!(error, UploadError::SizeMismatch { .. }));
}
