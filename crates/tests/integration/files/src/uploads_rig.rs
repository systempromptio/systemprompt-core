use std::sync::Arc;

use bytes::Bytes;
use sha2::{Digest, Sha256};
use systemprompt_files::documents::blob_store::BlobStore;
use systemprompt_files::documents::repository::DocumentBlobRepository;
use systemprompt_files::uploads::{
    CompletedUpload, DocumentUploadRepository, NewUpload, RefusalReason, UploadError, UploadResult,
    UploadService, UploadServiceParts,
};
use systemprompt_files::{DocumentsConfig, UploadLimits};
use systemprompt_identifiers::{ContentHash, TraceId, UserId};
use systemprompt_storage::LocalFileStorage;
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::{BackgroundTasks, FileByteStream, FileStorage};
use tempfile::TempDir;

pub(crate) struct UploadRig {
    pub(crate) service: UploadService,
    _dir: TempDir,
}

pub(crate) async fn rig_with(limits: UploadLimits) -> UploadRig {
    let db = test_db_pool().await;
    let dir = TempDir::new().expect("tempdir");
    let storage: Arc<dyn FileStorage> = Arc::new(LocalFileStorage::new(dir.path().to_path_buf()));
    let blobs = Arc::new(BlobStore::new(
        Arc::clone(&storage),
        Arc::new(DocumentBlobRepository::new(&db)),
    ));
    let service = UploadService::new(UploadServiceParts {
        storage,
        uploads: Arc::new(DocumentUploadRepository::new(&db)),
        blobs,
        max_upload_bytes: DocumentsConfig::default().max_upload_bytes,
        limits,
        tasks: BackgroundTasks::new(),
    });
    UploadRig { service, _dir: dir }
}

pub(crate) async fn rig() -> UploadRig {
    rig_with(UploadLimits::default()).await
}

pub(crate) fn owner() -> UserId {
    UserId::new(format!("wp4-uploader-{}", uuid::Uuid::new_v4().simple()))
}

pub(crate) fn trace() -> TraceId {
    TraceId::new(uuid::Uuid::new_v4().to_string())
}

pub(crate) fn hash_of(bytes: &[u8]) -> ContentHash {
    ContentHash::try_new(hex::encode(Sha256::digest(bytes))).expect("sha256 hex")
}

pub(crate) fn request(name: &str, mime: &str, bytes: &[u8]) -> NewUpload {
    NewUpload {
        name: name.to_owned(),
        mime: mime.to_owned(),
        size: bytes.len() as u64,
        sha256: hash_of(bytes),
    }
}

pub(crate) fn body(bytes: &[u8]) -> FileByteStream {
    Box::pin(futures::stream::iter([Ok(Bytes::copy_from_slice(bytes))]))
}

impl UploadRig {
    pub(crate) async fn put(&self, mime: &str, bytes: &[u8]) -> UploadResult<CompletedUpload> {
        self.service
            .put_inline(
                &owner(),
                request("upload.bin", mime, bytes),
                bytes.to_vec(),
                &trace(),
            )
            .await
    }
}

pub(crate) fn refusal(result: UploadResult<CompletedUpload>) -> RefusalReason {
    match result {
        Err(UploadError::Refused { reason, .. }) => reason,
        other => panic!("expected a refusal, got {other:?}"),
    }
}

pub(crate) fn refusal_detail(result: UploadResult<CompletedUpload>) -> (RefusalReason, String) {
    match result {
        Err(UploadError::Refused { reason, detail, .. }) => (reason, detail),
        other => panic!("expected a refusal, got {other:?}"),
    }
}
