use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicBool, Ordering};
use std::time::Duration;

use async_trait::async_trait;
use bytes::Bytes;
use futures::StreamExt;
use sha2::{Digest, Sha256};
use systemprompt_files::DocumentsConfig;
use systemprompt_files::documents::blob_store::BlobStore;
use systemprompt_files::documents::repository::DocumentBlobRepository;
use systemprompt_files::uploads::{
    DocumentUploadRepository, NewUpload, RefusalReason, UploadError, UploadService,
    UploadServiceParts,
};
use systemprompt_identifiers::{ContentHash, TraceId, UserId};
use systemprompt_storage::LocalFileStorage;
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::{
    BackgroundTasks, ByteRange, FileByteStream, FileStorage, FileStorageError, FileStorageResult,
    StoredFileId, StoredFileMetadata, StoredFileRead,
};
use tempfile::TempDir;

struct Faulty {
    inner: LocalFileStorage,
    stall_after_store: AtomicBool,
    fail_delete: AtomicBool,
}

#[async_trait]
impl FileStorage for Faulty {
    async fn store(&self, path: &Path, content: &[u8]) -> FileStorageResult<StoredFileId> {
        let stored = self.inner.store(path, content).await?;
        if self.stall_after_store.load(Ordering::SeqCst) {
            futures::future::pending::<()>().await;
        }
        Ok(stored)
    }

    async fn open(
        &self,
        id: &StoredFileId,
        range: Option<ByteRange>,
    ) -> FileStorageResult<StoredFileRead> {
        self.inner.open(id, range).await
    }

    async fn delete(&self, id: &StoredFileId) -> FileStorageResult<()> {
        if self.fail_delete.load(Ordering::SeqCst) {
            return Err(FileStorageError::Io(std::io::Error::other(
                "delete refused",
            )));
        }
        self.inner.delete(id).await
    }

    async fn metadata(&self, id: &StoredFileId) -> FileStorageResult<StoredFileMetadata> {
        self.inner.metadata(id).await
    }

    async fn exists(&self, id: &StoredFileId) -> FileStorageResult<bool> {
        self.inner.exists(id).await
    }
}

struct Rig {
    service: UploadService,
    uploads: Arc<DocumentUploadRepository>,
    storage: Arc<Faulty>,
    tasks: BackgroundTasks,
    db: systemprompt_database::DbPool,
    _dir: TempDir,
}

async fn rig() -> Rig {
    let db = test_db_pool().await;
    let dir = TempDir::new().expect("tempdir");
    let storage = Arc::new(Faulty {
        inner: LocalFileStorage::new(dir.path().to_path_buf()),
        stall_after_store: AtomicBool::new(false),
        fail_delete: AtomicBool::new(false),
    });
    let dyn_storage: Arc<dyn FileStorage> = Arc::clone(&storage) as Arc<dyn FileStorage>;
    let uploads = Arc::new(DocumentUploadRepository::new(&db));
    let blobs = Arc::new(BlobStore::new(
        Arc::clone(&dyn_storage),
        Arc::new(DocumentBlobRepository::new(&db)),
    ));
    let config = DocumentsConfig::default();
    let tasks = BackgroundTasks::new();
    let service = UploadService::new(UploadServiceParts {
        storage: dyn_storage,
        uploads: Arc::clone(&uploads),
        blobs,
        max_upload_bytes: config.max_upload_bytes,
        limits: config.uploads,
        tasks: tasks.clone(),
    });
    Rig {
        service,
        uploads,
        storage,
        tasks,
        db,
        _dir: dir,
    }
}

fn owner() -> UserId {
    UserId::new(format!("uploader-{}", uuid::Uuid::new_v4().simple()))
}

fn hash_of(bytes: &[u8]) -> ContentHash {
    ContentHash::try_new(hex::encode(Sha256::digest(bytes))).expect("sha256 hex")
}

fn request(name: &str, mime: &str, bytes: &[u8]) -> NewUpload {
    NewUpload {
        name: name.to_owned(),
        mime: mime.to_owned(),
        size: bytes.len() as u64,
        sha256: hash_of(bytes),
    }
}

fn body(bytes: &'static [u8]) -> FileByteStream {
    Box::pin(futures::stream::iter([Ok(Bytes::from_static(bytes))]))
}

fn stalled() -> FileByteStream {
    Box::pin(futures::stream::pending::<Result<Bytes, FileStorageError>>().boxed())
}

const PAYLOAD: &[u8] = b"sixteen byte bod";

#[tokio::test]
async fn a_dropped_part_attempt_refunds_its_reservation() {
    let rig = rig().await;
    let who = owner();
    let session = rig
        .service
        .create(
            &who,
            request("a.bin", "application/octet-stream", PAYLOAD),
            &TraceId::generate(),
        )
        .await
        .expect("session opens");
    for _ in 0..3 {
        let dropped = tokio::time::timeout(
            Duration::from_millis(100),
            rig.service.put_part(&who, &session.id, 1, stalled()),
        )
        .await;
        assert!(dropped.is_err(), "the stalled attempt must time out");
        assert!(
            rig.tasks.drain(Duration::from_secs(5)).await.is_drained(),
            "the refund task must finish"
        );
    }
    let stored = rig
        .service
        .put_part(&who, &session.id, 1, body(PAYLOAD))
        .await
        .expect("the part budget was refunded for every dropped attempt");
    assert_eq!(stored.size_bytes, PAYLOAD.len() as u64);
}

#[tokio::test]
async fn a_refund_for_an_attempt_whose_part_landed_is_skipped() {
    let rig = rig().await;
    let who = owner();
    let session = rig
        .service
        .create(
            &who,
            request("r.bin", "application/octet-stream", PAYLOAD),
            &TraceId::generate(),
        )
        .await
        .expect("session opens");
    let landed = rig
        .service
        .put_part(&who, &session.id, 1, body(PAYLOAD))
        .await
        .expect("part lands");
    let received = || async {
        let counted: i64 =
            sqlx::query_scalar("SELECT received_bytes FROM document_uploads WHERE id = $1::uuid")
                .bind(session.id.as_str())
                .fetch_one(&*rig.db.write_pool())
                .await
                .expect("received bytes");
        u64::try_from(counted).expect("non-negative")
    };
    let before = received().await;
    rig.uploads
        .refund_part_bytes(
            &session.id,
            Some(&StoredFileId::new(landed.storage_id.as_str())),
            landed.size_bytes,
        )
        .await
        .expect("refund");
    assert_eq!(received().await, before, "a landed attempt keeps its bytes");
    rig.uploads
        .refund_part_bytes(
            &session.id,
            Some(&StoredFileId::new(format!("{}-other", landed.storage_id))),
            landed.size_bytes,
        )
        .await
        .expect("refund");
    assert_eq!(
        received().await,
        before - landed.size_bytes,
        "an attempt that never landed is refunded"
    );
}

#[tokio::test]
async fn an_object_stored_before_an_interrupted_swap_is_swept() {
    let rig = rig().await;
    let who = owner();
    let session = rig
        .service
        .create(
            &who,
            request("b.bin", "application/octet-stream", PAYLOAD),
            &TraceId::generate(),
        )
        .await
        .expect("session opens");
    rig.storage.stall_after_store.store(true, Ordering::SeqCst);
    let dropped = tokio::time::timeout(
        Duration::from_millis(500),
        rig.service.put_part(&who, &session.id, 1, body(PAYLOAD)),
    )
    .await;
    assert!(
        dropped.is_err(),
        "the attempt must stall after its bytes are stored"
    );
    rig.storage.stall_after_store.store(false, Ordering::SeqCst);
    let orphan = rig
        .uploads
        .list_orphaned_objects(i64::MAX)
        .await
        .expect("orphans list");
    assert!(
        !orphan.iter().any(|object| object.upload_id == session.id),
        "an open upload's fresh attempt is not swept"
    );

    rig.service.abort(&who, &session.id).await.expect("abort");
    let orphans: Vec<_> = rig
        .uploads
        .list_orphaned_objects(i64::MAX)
        .await
        .expect("orphans list")
        .into_iter()
        .filter(|object| object.upload_id == session.id)
        .collect();
    assert_eq!(
        orphans.len(),
        1,
        "the stored attempt is recorded as an orphan"
    );
    assert!(
        rig.storage
            .exists(&orphans[0].storage_id)
            .await
            .expect("exists"),
        "the orphan's bytes were stored"
    );

    rig.service.sweep_orphans(i64::MAX).await.expect("sweep");
    assert!(
        !rig.storage
            .exists(&orphans[0].storage_id)
            .await
            .expect("exists"),
        "the sweep deleted the orphan"
    );
    let left = rig
        .uploads
        .list_orphaned_objects(i64::MAX)
        .await
        .expect("orphans list");
    assert!(!left.iter().any(|object| object.upload_id == session.id));
}

#[tokio::test]
async fn a_failed_delete_is_reported_and_kept_for_the_sweep() {
    let rig = rig().await;
    let who = owner();
    let session = rig
        .service
        .create(
            &who,
            request("c.bin", "application/octet-stream", PAYLOAD),
            &TraceId::generate(),
        )
        .await
        .expect("session opens");
    let part = rig
        .service
        .put_part(&who, &session.id, 1, body(PAYLOAD))
        .await
        .expect("part stored");

    rig.storage.fail_delete.store(true, Ordering::SeqCst);
    let removal = rig.service.abort(&who, &session.id).await.expect("abort");
    assert!(!removal.is_complete());
    assert_eq!(
        removal.retained,
        vec![StoredFileId::new(part.storage_id.as_str())]
    );
    let sweep = rig.service.sweep_orphans(i64::MAX).await.expect("sweep");
    assert!(
        sweep.retained >= 1,
        "a delete that still fails stays recorded"
    );

    rig.storage.fail_delete.store(false, Ordering::SeqCst);
    rig.service.sweep_orphans(i64::MAX).await.expect("sweep");
    assert!(
        !rig.storage
            .exists(&StoredFileId::new(part.storage_id.as_str()))
            .await
            .expect("exists")
    );
    let left = rig
        .uploads
        .list_orphaned_objects(i64::MAX)
        .await
        .expect("orphans list");
    assert!(!left.iter().any(|object| object.upload_id == session.id));
}

async fn svg_outcome(svg: &'static str) -> Result<(), UploadError> {
    let rig = rig().await;
    rig.service
        .put_inline(
            &owner(),
            request("drawing.svg", "image/svg+xml", svg.as_bytes()),
            svg.as_bytes().to_vec(),
            &TraceId::generate(),
        )
        .await
        .map(|_| ())
}

async fn assert_unsafe(svg: &'static str) {
    match svg_outcome(svg).await {
        Err(UploadError::Refused {
            reason: RefusalReason::UnsafeSvg,
            ..
        }) => {},
        other => panic!("expected an unsafe_svg refusal for {svg}, got {other:?}"),
    }
}

const SVG_OPEN: &str = r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 10 10">"#;

#[tokio::test]
async fn a_plain_svg_passes() {
    svg_outcome(concat!(
        r#"<?xml version="1.0" encoding="UTF-8"?>"#,
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink" viewBox="0 0 10 10">"#,
        r#"<defs><linearGradient id="g"><stop offset="0" stop-color="red"/></linearGradient></defs>"#,
        r#"<style>rect { fill: url(#g); }</style>"#,
        r#"<rect width="10" height="10" style="stroke: url( '#g' )"/>"#,
        r##"<use xlink:href="#g"/><text x="1" y="5">A &amp; B</text></svg>"##,
    ))
    .await
    .expect("a plain drawing passes");
}

#[tokio::test]
async fn svg_bypasses_are_refused() {
    let _ = SVG_OPEN;
    assert_unsafe(concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:xlink="http://www.w3.org/1999/xlink">"#,
        r#"<a xlink:href="&#106;avascript:alert(1)"><rect width="1" height="1"/></a></svg>"#,
    ))
    .await;
    assert_unsafe(concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
        r#"<a href=" javascript:alert(1)"><rect width="1" height="1"/></a></svg>"#,
    ))
    .await;
    assert_unsafe(concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg">"#,
        r#"<image href="//evil.example/x.png" width="1" height="1"/></svg>"#,
    ))
    .await;
    assert_unsafe(r#"<svg xmlns="http://www.w3.org/2000/svg" onload="alert(1)"></svg>"#).await;
    assert_unsafe(r#"<svg xmlns="http://www.w3.org/2000/svg"><rect ONclick ="x()"/></svg>"#).await;
    assert_unsafe(concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><foreignObject width="1" height="1">"#,
        r#"<div xmlns="http://www.w3.org/1999/xhtml">x</div></foreignObject></svg>"#,
    ))
    .await;
    assert_unsafe(r#"<svg xmlns="http://www.w3.org/2000/svg"><script>alert(1)</script></svg>"#)
        .await;
    assert_unsafe(concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg" xmlns:h="http://www.w3.org/1999/xhtml">"#,
        r#"<h:script>alert(1)</h:script></svg>"#,
    ))
    .await;
    assert_unsafe(concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><style>rect { fill: url(&#104;ttp://evil/x) }</style>"#,
        r#"</svg>"#,
    ))
    .await;
    assert_unsafe(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><rect style="fill:u\72l(http://evil)"/></svg>"#,
    )
    .await;
    assert_unsafe(concat!(
        r#"<svg xmlns="http://www.w3.org/2000/svg"><set attributeName="href" to="javascript:alert(1)"/>"#,
        r#"</svg>"#,
    ))
    .await;
    assert_unsafe(r#"<!DOCTYPE svg [<!ENTITY x "y">]><svg xmlns="http://www.w3.org/2000/svg"/>"#)
        .await;
}
