use std::path::{Path, PathBuf};
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_storage::{
    FileStorageBackend, GcsError, GcsFileStorage, GcsParams, GcsTokenSource, LocalFileStorage,
};
use systemprompt_traits::{ByteRange, FileStorage, FileStorageError, StoredFileId};
use tempfile::TempDir;
use url::Url;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct StaticTokens;

#[async_trait]
impl GcsTokenSource for StaticTokens {
    async fn bearer(&self) -> Result<String, GcsError> {
        Ok("tok".to_owned())
    }
}

fn gcs(server: &MockServer, prefix: Option<&str>) -> GcsFileStorage {
    let base = Url::parse(&server.uri()).expect("mock uri");
    GcsFileStorage::new(
        GcsParams::new("b", prefix.map(str::to_owned), false)
            .expect("params")
            .with_endpoints(base.clone(), base),
        Arc::new(StaticTokens),
        reqwest::Client::new(),
    )
}

async fn open_err(
    storage: &dyn FileStorage,
    id: &StoredFileId,
    range: Option<ByteRange>,
) -> FileStorageError {
    match storage.open(id, range).await {
        Ok(_) => panic!("open of {id:?} unexpectedly succeeded"),
        Err(err) => err,
    }
}

async fn mount_metadata(server: &MockServer, object: &str, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path(format!("/storage/v1/b/b/o/{object}")))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

#[tokio::test]
async fn gcs_object_name_prefixes_and_normalises_the_id() {
    let server = MockServer::start().await;
    let storage = gcs(&server, Some("tenant"));
    assert_eq!(
        storage
            .object_name(&StoredFileId::new("./files/a.png"))
            .expect("name"),
        "tenant/files/a.png"
    );
    assert!(storage.object_name(&StoredFileId::new("../x")).is_err());
    let rendered = format!("{storage:?}");
    assert!(
        rendered.contains("GcsFileStorage") && rendered.contains("tenant"),
        "{rendered}"
    );
}

#[tokio::test]
async fn gcs_a_416_with_content_range_reports_that_size() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/storage/v1/b/b/o/a.bin"))
        .and(query_param("alt", "media"))
        .respond_with(ResponseTemplate::new(416).insert_header("content-range", "bytes */10"))
        .mount(&server)
        .await;
    let storage = gcs(&server, None);
    let err = open_err(
        &storage,
        &StoredFileId::new("a.bin"),
        Some(ByteRange::From(50)),
    )
    .await;
    assert!(
        matches!(err, FileStorageError::RangeNotSatisfiable { size: 10 }),
        "{err:?}"
    );
}

#[tokio::test]
async fn gcs_a_416_without_content_range_is_sized_from_the_object_resource() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/storage/v1/b/b/o/a.bin"))
        .and(query_param("alt", "media"))
        .respond_with(ResponseTemplate::new(416))
        .with_priority(1)
        .mount(&server)
        .await;
    mount_metadata(
        &server,
        "a.bin",
        serde_json::json!({"size": "42", "timeCreated": "2026-10-01T10:00:00Z"}),
    )
    .await;
    let storage = gcs(&server, None);
    let err = open_err(
        &storage,
        &StoredFileId::new("a.bin"),
        Some(ByteRange::From(99)),
    )
    .await;
    assert!(
        matches!(err, FileStorageError::RangeNotSatisfiable { size: 42 }),
        "{err:?}"
    );
}

#[tokio::test]
async fn gcs_metadata_without_content_type_falls_back_to_the_extension() {
    let server = MockServer::start().await;
    mount_metadata(
        &server,
        "pic.png",
        serde_json::json!({"size": "3", "timeCreated": "2026-10-01T10:00:00Z"}),
    )
    .await;
    let meta = gcs(&server, None)
        .metadata(&StoredFileId::new("pic.png"))
        .await
        .expect("metadata");
    assert_eq!(meta.mime_type, "image/png");
    assert_eq!(
        meta.updated_at, meta.created_at,
        "no `updated` means created"
    );
    assert_eq!(meta.etag, None);
}

#[tokio::test]
async fn gcs_metadata_with_a_non_numeric_size_is_a_backend_error() {
    let server = MockServer::start().await;
    mount_metadata(
        &server,
        "a.bin",
        serde_json::json!({"size": "lots", "timeCreated": "2026-10-01T10:00:00Z"}),
    )
    .await;
    let err = gcs(&server, None)
        .metadata(&StoredFileId::new("a.bin"))
        .await
        .expect_err("bad size");
    assert!(matches!(err, FileStorageError::Backend(_)), "{err:?}");
}

#[tokio::test]
async fn gcs_metadata_without_time_created_is_a_backend_error() {
    let server = MockServer::start().await;
    mount_metadata(&server, "a.bin", serde_json::json!({"size": "3"})).await;
    let err = gcs(&server, None)
        .metadata(&StoredFileId::new("a.bin"))
        .await
        .expect_err("missing timeCreated");
    assert!(matches!(err, FileStorageError::Backend(_)), "{err:?}");
}

#[tokio::test]
async fn gcs_metadata_of_a_missing_object_is_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/storage/v1/b/b/o/gone.bin"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let err = gcs(&server, None)
        .metadata(&StoredFileId::new("gone.bin"))
        .await
        .expect_err("missing");
    assert!(
        matches!(err, FileStorageError::NotFound(ref id) if id == "gone.bin"),
        "{err:?}"
    );
}

fn local() -> (TempDir, LocalFileStorage) {
    let dir = TempDir::new().expect("tempdir");
    let storage = LocalFileStorage::new(dir.path().to_path_buf());
    (dir, storage)
}

#[tokio::test]
async fn local_missing_files_are_not_found_for_every_operation() {
    let (_dir, storage) = local();
    let id = StoredFileId::new("files/none.txt");

    assert!(matches!(
        open_err(&storage, &id, None).await,
        FileStorageError::NotFound(_)
    ));
    assert!(matches!(
        storage.delete(&id).await.expect_err("delete"),
        FileStorageError::NotFound(_)
    ));
    assert!(matches!(
        storage.metadata(&id).await.expect_err("metadata"),
        FileStorageError::NotFound(_)
    ));
    assert!(!storage.exists(&id).await.expect("exists"));
}

#[tokio::test]
async fn local_exists_tracks_store_and_delete() {
    let (_dir, storage) = local();
    let id = storage
        .store(Path::new("a/b.txt"), b"hello")
        .await
        .expect("store");
    assert!(storage.exists(&id).await.expect("exists"));
    storage.delete(&id).await.expect("delete");
    assert!(!storage.exists(&id).await.expect("exists after delete"));
}

#[tokio::test]
async fn local_store_leaves_no_staging_file_behind() {
    let (dir, storage) = local();
    storage
        .store(Path::new("files/x.json"), b"{}")
        .await
        .expect("store");
    let names: Vec<String> = std::fs::read_dir(dir.path().join("files"))
        .expect("read dir")
        .map(|e| e.expect("entry").file_name().to_string_lossy().into_owned())
        .collect();
    assert_eq!(names, vec!["x.json".to_owned()]);
}

#[tokio::test]
async fn local_delete_of_a_directory_is_a_backend_error_not_not_found() {
    let (dir, storage) = local();
    std::fs::create_dir_all(dir.path().join("adir")).expect("mkdir");
    let err = storage
        .delete(&StoredFileId::new("adir"))
        .await
        .expect_err("cannot unlink a directory");
    assert!(matches!(err, FileStorageError::Backend(_)), "{err:?}");
    let source = std::error::Error::source(&err).expect("cause kept");
    assert!(source.to_string().contains("adir"), "{source}");
}

#[test]
fn backend_debug_names_the_variant_without_credentials() {
    let local = FileStorageBackend::Local {
        root: PathBuf::from("/srv/files"),
    };
    let rendered = format!("{local:?}");
    assert!(
        rendered.contains("Local") && rendered.contains("/srv/files"),
        "{rendered}"
    );

    let gcs = FileStorageBackend::Gcs {
        params: Box::new(GcsParams::new("bkt", None, true).expect("params")),
        tokens: Arc::new(StaticTokens),
        http: reqwest::Client::new(),
    };
    let rendered = format!("{gcs:?}");
    assert!(
        rendered.starts_with("Gcs") && rendered.contains("bkt"),
        "{rendered}"
    );
}
