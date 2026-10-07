use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_storage::{
    FileStorageBackend, GcsError, GcsFileStorage, GcsParams, GcsTokenSource, build_file_storage,
};
use systemprompt_traits::{FileStorage, FileStorageError, StoredFileId};
use url::Url;
use wiremock::matchers::{body_bytes, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

struct StaticTokens;

#[async_trait]
impl GcsTokenSource for StaticTokens {
    async fn bearer(&self) -> Result<String, GcsError> {
        Ok("tok-1".to_owned())
    }
}

fn params(server: &MockServer, prefix: Option<&str>, public_read: bool) -> GcsParams {
    let base = Url::parse(&server.uri()).unwrap();
    GcsParams::new("my-bucket", prefix.map(str::to_owned), public_read)
        .unwrap()
        .with_endpoints(base.clone(), base)
}

fn storage(server: &MockServer, prefix: Option<&str>) -> Arc<dyn FileStorage> {
    build_file_storage(FileStorageBackend::Gcs {
        params: Box::new(params(server, prefix, false)),
        tokens: Arc::new(StaticTokens),
        http: reqwest::Client::new(),
    })
}

#[tokio::test]
async fn store_uploads_media_under_the_prefix_with_a_bearer() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/upload/storage/v1/b/my-bucket/o"))
        .and(query_param("uploadType", "media"))
        .and(query_param("name", "tenant-a/files/uploads/a.png"))
        .and(header("authorization", "Bearer tok-1"))
        .and(header("content-type", "image/png"))
        .and(body_bytes(b"png-bytes".to_vec()))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&server)
        .await;
    let id = storage(&server, Some("tenant-a/files"))
        .store(Path::new("./uploads/a.png"), b"png-bytes")
        .await
        .unwrap();
    assert_eq!(id.as_str(), "uploads/a.png");
}

#[tokio::test]
async fn retrieve_downloads_the_media_with_the_object_name_encoded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/storage/v1/b/my-bucket/o/p%2Fuploads%2Fa.txt"))
        .and(query_param("alt", "media"))
        .and(header("authorization", "Bearer tok-1"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"hello".to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    let bytes = storage(&server, Some("p"))
        .retrieve(&StoredFileId::new("uploads/a.txt"))
        .await
        .unwrap();
    assert_eq!(bytes, b"hello");
}

#[tokio::test]
async fn delete_removes_the_object_and_a_404_is_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/storage/v1/b/my-bucket/o/gone.txt"))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("DELETE"))
        .and(path("/storage/v1/b/my-bucket/o/missing.txt"))
        .respond_with(ResponseTemplate::new(404).set_body_string("No such object"))
        .mount(&server)
        .await;
    let s = storage(&server, None);
    s.delete(&StoredFileId::new("gone.txt")).await.unwrap();
    let err = s
        .delete(&StoredFileId::new("missing.txt"))
        .await
        .unwrap_err();
    assert!(
        matches!(err, FileStorageError::NotFound(ref id) if id == "missing.txt"),
        "{err:?}"
    );
}

#[tokio::test]
async fn metadata_parses_size_mime_and_timestamps() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/storage/v1/b/my-bucket/o/doc.pdf"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "name": "doc.pdf",
            "size": "2048",
            "contentType": "application/pdf",
            "timeCreated": "2026-10-01T10:00:00Z",
            "updated": "2026-10-02T11:30:00Z"
        })))
        .mount(&server)
        .await;
    let meta = storage(&server, None)
        .metadata(&StoredFileId::new("doc.pdf"))
        .await
        .unwrap();
    assert_eq!(meta.size_bytes, Some(2048));
    assert_eq!(meta.mime_type, "application/pdf");
    assert_eq!(meta.path, "doc.pdf");
    assert_eq!(meta.created_at.to_rfc3339(), "2026-10-01T10:00:00+00:00");
    assert_eq!(meta.updated_at.to_rfc3339(), "2026-10-02T11:30:00+00:00");
}

#[tokio::test]
async fn exists_is_true_for_an_object_and_false_for_a_404() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/storage/v1/b/my-bucket/o/here.txt"))
        .respond_with(ResponseTemplate::new(200).set_body_json(serde_json::json!({
            "size": "1", "timeCreated": "2026-10-01T10:00:00Z"
        })))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/storage/v1/b/my-bucket/o/absent.txt"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let s = storage(&server, None);
    assert!(s.exists(&StoredFileId::new("here.txt")).await.unwrap());
    assert!(!s.exists(&StoredFileId::new("absent.txt")).await.unwrap());
}

#[tokio::test]
async fn a_403_is_a_backend_error_carrying_the_status() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(403).set_body_string("denied"))
        .mount(&server)
        .await;
    let err = storage(&server, None)
        .retrieve(&StoredFileId::new("x.txt"))
        .await
        .unwrap_err();
    let FileStorageError::Backend(source) = err else {
        panic!("expected Backend, got {err:?}");
    };
    let gcs = source.downcast_ref::<GcsError>().expect("GcsError source");
    assert!(
        matches!(gcs, GcsError::Status { status: 403, body } if body == "denied"),
        "{gcs:?}"
    );
}

#[tokio::test]
async fn parent_components_are_refused_before_any_request() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .expect(0)
        .mount(&server)
        .await;
    let s = storage(&server, Some("p"));
    for bad in ["../escape.txt", "a/../../b", "/abs.txt"] {
        let err = s.retrieve(&StoredFileId::new(bad)).await.unwrap_err();
        assert!(
            matches!(err, FileStorageError::Validation(_)),
            "{bad}: {err:?}"
        );
    }
    let err = s.store(Path::new("../up.txt"), b"x").await.unwrap_err();
    assert!(matches!(err, FileStorageError::Validation(_)), "{err:?}");
}

#[tokio::test]
async fn public_url_exists_only_with_public_read() {
    let server = MockServer::start().await;
    let private = GcsFileStorage::new(
        params(&server, Some("pre fix"), false),
        Arc::new(StaticTokens),
        reqwest::Client::new(),
    );
    assert_eq!(private.public_url(&StoredFileId::new("a.png")), None);

    let public = GcsFileStorage::new(
        params(&server, Some("pre fix"), true),
        Arc::new(StaticTokens),
        reqwest::Client::new(),
    );
    assert_eq!(
        public.public_url(&StoredFileId::new("img/a b.png")),
        Some(format!(
            "{}/my-bucket/pre%20fix/img/a%20b.png",
            server.uri()
        ))
    );
    assert_eq!(public.public_url(&StoredFileId::new("../x")), None);
}

#[tokio::test]
async fn default_endpoints_name_the_public_storage_host() {
    let p = GcsParams::new("b-1", None, true).unwrap();
    assert_eq!(p.api_base.as_str(), "https://storage.googleapis.com/");
    let s = GcsFileStorage::new(p, Arc::new(StaticTokens), reqwest::Client::new());
    assert_eq!(
        s.public_url(&StoredFileId::new("a/b.png")).as_deref(),
        Some("https://storage.googleapis.com/b-1/a/b.png")
    );
}
