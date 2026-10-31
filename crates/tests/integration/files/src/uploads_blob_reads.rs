use futures::StreamExt;
use systemprompt_files::uploads::{
    CompletedUpload, RefusalReason, UploadError, UploadResult, UploadStatus,
};
use systemprompt_identifiers::{ContentHash, UserId};
use systemprompt_models::documents::DocumentFormat;
use systemprompt_test_fixtures::test_db_pool;
use uuid::Uuid;

use crate::uploads_rig::{UploadRig, hash_of, request, rig, trace};

const TEXT: &str = "text/plain";

fn uploader() -> UserId {
    UserId::new("wp9-blob-reader")
}

async fn put(rig: &UploadRig, bytes: &[u8]) -> UploadResult<CompletedUpload> {
    rig.service
        .put_inline(
            &uploader(),
            request("upload.bin", TEXT, bytes),
            bytes.to_vec(),
            &trace(),
        )
        .await
}

fn stranger() -> UserId {
    UserId::new(format!("stranger-{}", Uuid::new_v4()))
}

fn distinct(label: &str) -> Vec<u8> {
    format!("{label} {}\n", Uuid::new_v4()).into_bytes()
}

async fn forget_checked_kind(blob: &ContentHash) {
    let pool = test_db_pool().await.pool();
    sqlx::query("UPDATE document_uploads SET checked_kind = NULL WHERE blob_sha256 = $1")
        .bind(blob.as_str())
        .execute(&*pool)
        .await
        .expect("forget checked kind");
}

async fn checked_kind(blob: &ContentHash) -> Option<String> {
    let pool = test_db_pool().await.pool();
    sqlx::query_scalar(
        "SELECT checked_kind FROM document_uploads WHERE blob_sha256 = $1 AND status = 'completed' \
         LIMIT 1",
    )
    .bind(blob.as_str())
    .fetch_one(&*pool)
    .await
    .expect("checked kind")
}

#[tokio::test]
async fn only_the_uploader_owns_a_blob() {
    let rig = rig().await;
    let bytes = distinct("owned");
    let done = put(&rig, &bytes).await.expect("upload");

    let origin = rig
        .service
        .require_owned_blob(&uploader(), &done.blob)
        .await
        .expect("owner");
    assert_eq!(origin.checked_kind.as_deref(), Some("text"));

    let refused = rig
        .service
        .require_owned_blob(&stranger(), &done.blob)
        .await
        .expect_err("stranger");
    assert!(
        matches!(&refused, UploadError::BlobNotOwned(blob) if *blob == done.blob),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_text_blob_imports_as_markdown_but_not_as_office() {
    let rig = rig().await;
    let bytes = distinct("import");
    let done = put(&rig, &bytes).await.expect("upload");

    let (read, origin) = rig
        .service
        .read_owned_blob(&uploader(), &done.blob, DocumentFormat::Markdown, 1 << 20)
        .await
        .expect("markdown import");
    assert_eq!(read, bytes);
    assert_eq!(origin.checked_kind.as_deref(), Some("text"));

    let refused = rig
        .service
        .read_owned_blob(&uploader(), &done.blob, DocumentFormat::Docx, 1 << 20)
        .await
        .expect_err("not a zip");
    assert!(
        matches!(
            refused,
            UploadError::Refused {
                reason: RefusalReason::UncheckedFormat,
                ..
            }
        ),
        "{refused:?}"
    );
}

#[tokio::test]
async fn a_blob_past_the_read_limit_is_refused_not_truncated() {
    let rig = rig().await;
    let bytes = distinct("a blob longer than the limit allows");
    let done = put(&rig, &bytes).await.expect("upload");

    let refused = rig
        .service
        .read_owned_blob(&uploader(), &done.blob, DocumentFormat::Text, 4)
        .await
        .expect_err("too large");
    assert!(
        matches!(refused, UploadError::TooLarge { limit: 4, size } if size == bytes.len() as u64),
        "{refused:?}"
    );
}

#[tokio::test]
async fn an_unchecked_blob_is_inspected_on_read_and_the_kind_recorded() {
    let rig = rig().await;
    let bytes = distinct("recheck");
    let done = put(&rig, &bytes).await.expect("upload");
    forget_checked_kind(&done.blob).await;
    assert_eq!(checked_kind(&done.blob).await, None);

    let (read, origin) = rig
        .service
        .read_owned_blob(&uploader(), &done.blob, DocumentFormat::Text, 1 << 20)
        .await
        .expect("rechecked import");
    assert_eq!(read, bytes);
    assert_eq!(origin.checked_kind.as_deref(), Some("text"));
    assert_eq!(checked_kind(&done.blob).await.as_deref(), Some("text"));
}

#[tokio::test]
async fn a_completed_upload_streams_back_and_an_open_one_is_closed_to_reads() {
    let rig = rig().await;
    let bytes = distinct("download");
    let done = put(&rig, &bytes).await.expect("upload");

    let (session, read) = rig
        .service
        .open_blob(&uploader(), &done.session.id, None)
        .await
        .expect("open");
    assert_eq!(session.id, done.session.id);
    let mut stream = read.body;
    let mut streamed = Vec::new();
    while let Some(chunk) = stream.next().await {
        streamed.extend_from_slice(&chunk.expect("chunk"));
    }
    assert_eq!(streamed, bytes);
    assert_eq!(hash_of(&streamed), done.blob);

    let pending = distinct("pending");
    let open = rig
        .service
        .create(
            &uploader(),
            request("pending.txt", TEXT, &pending),
            &trace(),
        )
        .await
        .expect("create");
    let refused = rig
        .service
        .open_blob(&uploader(), &open.id, None)
        .await
        .expect_err("still open");
    assert!(
        matches!(
            refused,
            UploadError::Closed {
                status: UploadStatus::Open
            }
        ),
        "{refused:?}"
    );
}
