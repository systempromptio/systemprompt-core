//! `BlobStore` over local storage and the test database. Blob GC scenarios
//! share one test so no concurrent GC pass in this binary collects another
//! test's aged blobs.

use std::path::Path;
use std::sync::Arc;
use std::sync::atomic::{AtomicU64, Ordering};

use async_trait::async_trait;
use systemprompt_files::documents::blob_store::{BlobStore, BlobStoreError, storage_id_for};
use systemprompt_files::documents::models::{NewDocument, NewRevision, RevisionAttribution};
use systemprompt_files::documents::repository::DocumentRepositories;
use systemprompt_identifiers::{ContentHash, UserId};
use systemprompt_models::documents::{ActorKind, DocumentFormat, RevisionOriginKind};
use systemprompt_storage::{FileStorageBackend, build_file_storage};
use systemprompt_test_fixtures::test_db_pool;
use systemprompt_traits::{
    ByteRange, FileStorage, FileStorageError, FileStorageResult, StoredFileId, StoredFileMetadata,
    StoredFileRead,
};
use tempfile::TempDir;
use uuid::Uuid;

struct Harness {
    _root: TempDir,
    storage: Arc<dyn FileStorage>,
    repos: DocumentRepositories,
    pool: systemprompt_database::DbPool,
}

async fn harness() -> Harness {
    let root = tempfile::tempdir().expect("tempdir");
    let storage = build_file_storage(FileStorageBackend::Local {
        root: root.path().to_path_buf(),
    });
    let pool = test_db_pool().await;
    Harness {
        _root: root,
        storage,
        repos: DocumentRepositories::new(&pool),
        pool,
    }
}

impl Harness {
    fn store(&self) -> BlobStore {
        BlobStore::new(Arc::clone(&self.storage), Arc::clone(&self.repos.blobs))
    }

    async fn age(&self, hash: &ContentHash, hours: i32) {
        sqlx::query(
            "UPDATE document_blobs SET created_at = now() - make_interval(hours => $2)
             WHERE content_sha256 = $1",
        )
        .bind(hash.as_str())
        .bind(hours)
        .execute(&*self.pool.write_pool())
        .await
        .expect("age blob");
    }

    async fn reference(&self, hash: &ContentHash) {
        let owner = UserId::new(format!("blob-owner-{}", Uuid::new_v4().simple()));
        let document = self
            .repos
            .documents
            .insert(NewDocument {
                folder_id: None,
                name: format!("blob-{}.md", Uuid::new_v4().simple()),
                format: DocumentFormat::Markdown,
                mime_type: "text/markdown".to_owned(),
                template_id: None,
                owner_user_id: owner.clone(),
                created_by: owner.clone(),
            })
            .await
            .expect("insert document");
        self.repos
            .revisions
            .insert(NewRevision {
                document_id: document.id,
                parent_revision_id: None,
                content_sha256: hash.clone(),
                author_user_id: owner,
                actor_kind: ActorKind::User,
                origin_kind: RevisionOriginKind::Api,
                origin_id: None,
                message: None,
                edit_ops: None,
                attribution: RevisionAttribution::default(),
            })
            .await
            .expect("insert revision");
    }
}

fn unique_bytes(label: &str) -> Vec<u8> {
    format!("{label}-{}", Uuid::new_v4()).into_bytes()
}

#[tokio::test]
async fn put_twice_reuses_the_blob() {
    let h = harness().await;
    let store = h.store();
    let bytes = unique_bytes("twice");
    let first = store.put(&bytes).await.expect("first put");
    let second = store.put(&bytes).await.expect("second put");
    assert!(first.inserted);
    assert!(!second.inserted);
    assert_eq!(first.storage_id, second.storage_id);
    assert_eq!(first.storage_id, storage_id_for(&first.hash));
    assert_eq!(first.size_bytes, bytes.len() as u64);
    let read = store.read_all(&first.hash, 1024).await.expect("read back");
    assert_eq!(read, bytes);
}

#[tokio::test]
async fn read_all_enforces_the_cap() {
    let h = harness().await;
    let store = h.store();
    let stored = store.put(&unique_bytes("capped")).await.expect("put");
    let result = store.read_all(&stored.hash, 4).await;
    assert!(matches!(
        result,
        Err(BlobStoreError::TooLarge { limit: 4, .. })
    ));
}

#[tokio::test]
async fn open_of_unknown_hash_is_not_found() {
    let h = harness().await;
    let unknown = ContentHash::new(format!(
        "{}{}",
        Uuid::new_v4().simple(),
        Uuid::new_v4().simple()
    ));
    let result = h.store().open(&unknown, None).await;
    assert!(matches!(result, Err(BlobStoreError::NotFound { .. })));
}

#[tokio::test]
async fn object_without_row_is_never_served() {
    let h = harness().await;
    let bytes = unique_bytes("orphan");
    let hash = systemprompt_models::documents::content_hash(&bytes);
    h.storage
        .store(Path::new(storage_id_for(&hash).as_str()), &bytes)
        .await
        .expect("store orphan object");
    let result = h.store().open(&hash, None).await;
    assert!(matches!(result, Err(BlobStoreError::NotFound { .. })));
    let restored = h.store().put(&bytes).await.expect("put restores row");
    assert!(restored.inserted);
}

struct FailingDelete {
    inner: Arc<dyn FileStorage>,
    attempts: AtomicU64,
}

#[async_trait]
impl FileStorage for FailingDelete {
    async fn store(&self, path: &Path, content: &[u8]) -> FileStorageResult<StoredFileId> {
        self.inner.store(path, content).await
    }

    async fn open(
        &self,
        id: &StoredFileId,
        range: Option<ByteRange>,
    ) -> FileStorageResult<StoredFileRead> {
        self.inner.open(id, range).await
    }

    async fn delete(&self, _id: &StoredFileId) -> FileStorageResult<()> {
        self.attempts.fetch_add(1, Ordering::SeqCst);
        Err(FileStorageError::Validation("delete refused".to_owned()))
    }

    async fn metadata(&self, id: &StoredFileId) -> FileStorageResult<StoredFileMetadata> {
        self.inner.metadata(id).await
    }

    async fn exists(&self, id: &StoredFileId) -> FileStorageResult<bool> {
        self.inner.exists(id).await
    }
}

#[tokio::test]
async fn gc_collects_only_aged_unreferenced_blobs_and_counts_failures() {
    let h = harness().await;
    let store = h.store();

    let aged = store.put(&unique_bytes("aged")).await.expect("aged");
    let referenced = store
        .put(&unique_bytes("referenced"))
        .await
        .expect("referenced");
    let young = store.put(&unique_bytes("young")).await.expect("young");
    h.age(&aged.hash, 48).await;
    h.age(&referenced.hash, 48).await;
    h.reference(&referenced.hash).await;

    let report = store.delete_unreferenced(24, 500).await.expect("gc");
    assert!(report.deleted >= 1);
    assert!(
        h.repos
            .blobs
            .find(&aged.hash)
            .await
            .expect("find")
            .is_none()
    );
    assert!(!h.storage.exists(&aged.storage_id).await.expect("exists"));
    assert!(
        h.repos
            .blobs
            .find(&referenced.hash)
            .await
            .expect("find")
            .is_some()
    );
    assert!(
        h.storage
            .exists(&referenced.storage_id)
            .await
            .expect("exists")
    );
    assert!(
        h.repos
            .blobs
            .find(&young.hash)
            .await
            .expect("find")
            .is_some()
    );
    assert!(h.storage.exists(&young.storage_id).await.expect("exists"));

    let failing = Arc::new(FailingDelete {
        inner: Arc::clone(&h.storage),
        attempts: AtomicU64::new(0),
    });
    let failing_store = BlobStore::new(
        Arc::clone(&failing) as Arc<dyn FileStorage>,
        Arc::clone(&h.repos.blobs),
    );
    let stuck = failing_store
        .put(&unique_bytes("stuck"))
        .await
        .expect("stuck");
    h.age(&stuck.hash, 48).await;
    let report = failing_store
        .delete_unreferenced(24, 500)
        .await
        .expect("gc");
    assert!(report.failed >= 1);
    assert!(failing.attempts.load(Ordering::SeqCst) >= 1);
    assert!(
        h.repos
            .blobs
            .find(&stuck.hash)
            .await
            .expect("find")
            .is_none()
    );
    assert!(h.storage.exists(&stuck.storage_id).await.expect("exists"));
}
