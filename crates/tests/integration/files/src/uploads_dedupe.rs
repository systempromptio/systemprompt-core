use systemprompt_files::uploads::UploadError;
use systemprompt_identifiers::UserId;

use crate::uploads_rig::{body, hash_of, request, rig, trace};

const TEXT: &str = "text/plain";

#[tokio::test]
async fn a_second_session_with_the_same_bytes_adopts_the_stored_blob() {
    let rig = rig().await;
    let user = UserId::new(format!("wp10-dedupe-{}", uuid::Uuid::new_v4()));
    let bytes = format!("shared content {}", uuid::Uuid::new_v4()).into_bytes();

    let mut completed = Vec::new();
    for name in ["first.txt", "second.txt"] {
        let session = rig
            .service
            .create(&user, request(name, TEXT, &bytes), &trace())
            .await
            .expect("create");
        rig.service
            .put_part(&user, &session.id, 1, body(&bytes))
            .await
            .expect("part");
        completed.push(
            rig.service
                .complete(&user, &session.id)
                .await
                .expect("complete"),
        );
    }

    assert!(!completed[0].deduplicated);
    assert!(
        completed[1].deduplicated,
        "the second upload reuses the blob"
    );
    assert_eq!(completed[0].blob, hash_of(&bytes));
    assert_eq!(completed[1].blob, completed[0].blob);
    let sizes: Vec<u64> = completed.iter().map(|c| c.size_bytes).collect();
    assert_eq!(sizes, vec![bytes.len() as u64; 2]);
}

#[tokio::test]
async fn completing_an_unknown_session_is_refused() {
    let rig = rig().await;
    let user = UserId::new("wp10-dedupe-unknown");
    let missing = systemprompt_identifiers::DocumentUploadId::new(uuid::Uuid::new_v4().to_string());

    let refused = rig
        .service
        .complete(&user, &missing)
        .await
        .expect_err("unknown");

    assert!(matches!(refused, UploadError::NotFound), "{refused:?}");
}
