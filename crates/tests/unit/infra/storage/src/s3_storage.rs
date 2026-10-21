use crate::support::ReadAll;
use std::path::Path;
use std::sync::Arc;

use systemprompt_storage::{
    FileStorageBackend, S3Credentials, S3Error, S3FileStorage, S3Params, build_file_storage,
};
use systemprompt_traits::{FileStorage, FileStorageError, StoredFileId};
use wiremock::matchers::{body_bytes, header, header_regex, method, path};
use wiremock::{Mock, MockServer, Request, ResponseTemplate};


fn credentials(session_token: Option<&str>) -> S3Credentials {
    serde_json::from_value(serde_json::json!({
        "access_key_id": "AKIDEXAMPLE",
        "secret_access_key": "wJalrXUtnFEMI/K7MDENG+bPxRfiCYEXAMPLEKEY",
        "session_token": session_token,
    }))
    .unwrap()
}

fn params(server: &MockServer, prefix: Option<&str>) -> S3Params {
    S3Params::new(
        "my-bucket",
        Some("eu-west-1".to_owned()),
        Some(&server.uri()),
    )
    .unwrap()
    .with_prefix(prefix.map(str::to_owned))
    .with_path_style(true)
}

fn storage(server: &MockServer, prefix: Option<&str>) -> Arc<dyn FileStorage> {
    build_file_storage(FileStorageBackend::S3 {
        params: Box::new(params(server, prefix)),
        credentials: credentials(None),
        http: reqwest::Client::new(),
    })
}

const AUTH: &str = r"^AWS4-HMAC-SHA256 Credential=AKIDEXAMPLE/\d{8}/eu-west-1/s3/aws4_request, SignedHeaders=host;x-amz-content-sha256;x-amz-date, Signature=[0-9a-f]{64}$";

fn sha256_hex(data: &[u8]) -> String {
    use sha2::{Digest, Sha256};
    hex::encode(Sha256::digest(data))
}

#[tokio::test]
async fn store_puts_the_object_path_style_with_a_sigv4_signature() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path("/my-bucket/tenant-a/files/uploads/a.png"))
        .and(header("content-type", "image/png"))
        .and(header(
            "x-amz-content-sha256",
            sha256_hex(b"png-bytes").as_str(),
        ))
        .and(header_regex("x-amz-date", r"^\d{8}T\d{6}Z$"))
        .and(header_regex("authorization", AUTH))
        .and(body_bytes(b"png-bytes".to_vec()))
        .respond_with(ResponseTemplate::new(200))
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
async fn retrieve_gets_the_object_with_the_key_encoded() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/my-bucket/files/uploads/a%20b.png"))
        .and(header("x-amz-content-sha256", sha256_hex(b"").as_str()))
        .and(header_regex("authorization", AUTH))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"bytes".to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    let bytes = storage(&server, None)
        .read_all(&StoredFileId::new("files/uploads/a b.png"))
        .await
        .unwrap();
    assert_eq!(bytes, b"bytes");
}

#[tokio::test]
async fn a_missing_object_is_not_found() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(404).set_body_string("<Error/>"))
        .mount(&server)
        .await;
    Mock::given(method("HEAD"))
        .respond_with(ResponseTemplate::new(404))
        .mount(&server)
        .await;
    let s = storage(&server, None);
    let id = StoredFileId::new("files/x.png");
    assert!(matches!(
        s.read_all(&id).await,
        Err(FileStorageError::NotFound(_))
    ));
    assert!(!s.exists(&id).await.unwrap());
    assert!(matches!(
        s.metadata(&id).await,
        Err(FileStorageError::NotFound(_))
    ));
}

#[tokio::test]
async fn an_oversized_error_body_is_capped_before_it_reaches_the_error() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(500).set_body_string("y".repeat(64 * 1024)))
        .mount(&server)
        .await;
    let err = storage(&server, None)
        .read_all(&StoredFileId::new("files/x.png"))
        .await
        .unwrap_err();
    let FileStorageError::Backend(source) = err else {
        panic!("expected Backend, got {err:?}");
    };
    let s3 = source.downcast_ref::<S3Error>().expect("S3Error source");
    let S3Error::Status { status, body } = s3 else {
        panic!("expected Status, got {s3:?}");
    };
    assert_eq!(*status, 500);
    assert!(body.len() <= 2048 + " [truncated]".len(), "{}", body.len());
    assert!(body.ends_with(" [truncated]"), "a cut body says so");
}

#[tokio::test]
async fn metadata_reads_the_head_response() {
    let server = MockServer::start().await;
    Mock::given(method("HEAD"))
        .and(path("/my-bucket/files/doc.pdf"))
        .respond_with(
            ResponseTemplate::new(200)
                .insert_header("content-type", "application/pdf")
                .insert_header("content-length", "1234")
                .insert_header("last-modified", "Wed, 14 Oct 2026 09:30:00 GMT"),
        )
        .mount(&server)
        .await;
    let meta = storage(&server, None)
        .metadata(&StoredFileId::new("files/doc.pdf"))
        .await
        .unwrap();
    assert_eq!(meta.mime_type, "application/pdf");
    assert_eq!(meta.size_bytes, Some(1234));
    assert_eq!(meta.updated_at.to_rfc3339(), "2026-10-14T09:30:00+00:00");
}

#[tokio::test]
async fn delete_issues_a_signed_delete() {
    let server = MockServer::start().await;
    Mock::given(method("DELETE"))
        .and(path("/my-bucket/files/a.png"))
        .and(header_regex("authorization", AUTH))
        .respond_with(ResponseTemplate::new(204))
        .expect(1)
        .mount(&server)
        .await;
    storage(&server, None)
        .delete(&StoredFileId::new("files/a.png"))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_session_token_is_sent_and_signed() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(header("x-amz-security-token", "sess-1"))
        .and(header_regex(
            "authorization",
            "SignedHeaders=host;x-amz-content-sha256;x-amz-date;x-amz-security-token,",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"ok".to_vec()))
        .expect(1)
        .mount(&server)
        .await;
    let s = S3FileStorage::new(
        params(&server, None),
        credentials(Some("sess-1")),
        reqwest::Client::new(),
    );
    assert_eq!(
        s.read_all(&StoredFileId::new("files/a")).await.unwrap(),
        b"ok"
    );
}

#[tokio::test]
async fn signatures_differ_per_object() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200))
        .mount(&server)
        .await;
    let s = storage(&server, None);
    s.read_all(&StoredFileId::new("files/a")).await.unwrap();
    s.read_all(&StoredFileId::new("files/b")).await.unwrap();
    let requests: Vec<Request> = server.received_requests().await.unwrap();
    let auth = |r: &Request| {
        r.headers
            .get("authorization")
            .unwrap()
            .to_str()
            .unwrap()
            .to_owned()
    };
    assert_ne!(auth(&requests[0]), auth(&requests[1]));
}

#[test]
fn path_traversal_is_refused_before_any_request() {
    let s = S3FileStorage::new(
        S3Params::new("b", None, Some("http://127.0.0.1:1")).unwrap(),
        credentials(None),
        reqwest::Client::new(),
    );
    assert!(matches!(
        s.object_name(&StoredFileId::new("../etc/passwd")),
        Err(FileStorageError::Validation(_))
    ));
}

#[test]
fn public_url_is_virtual_hosted_unless_path_style() {
    let base = S3Params::new("my-bucket", None, None)
        .unwrap()
        .with_public_read(true);
    let virtual_hosted =
        S3FileStorage::new(base.clone(), credentials(None), reqwest::Client::new());
    assert_eq!(
        virtual_hosted
            .public_url(&StoredFileId::new("files/a b.png"))
            .as_deref(),
        Some("https://my-bucket.s3.us-east-1.amazonaws.com/files/a%20b.png")
    );
    let path_style = S3FileStorage::new(
        base.with_path_style(true),
        credentials(None),
        reqwest::Client::new(),
    );
    assert_eq!(
        path_style
            .public_url(&StoredFileId::new("files/a.png"))
            .as_deref(),
        Some("https://s3.us-east-1.amazonaws.com/my-bucket/files/a.png")
    );
    let private = S3FileStorage::new(
        S3Params::new("my-bucket", None, None).unwrap(),
        credentials(None),
        reqwest::Client::new(),
    );
    assert!(
        private
            .public_url(&StoredFileId::new("files/a.png"))
            .is_none()
    );
}

#[test]
fn credentials_debug_never_prints_the_secret() {
    let rendered = format!("{:?}", credentials(Some("sess-1")));
    assert!(rendered.contains("AKIDEXAMPLE"));
    assert!(!rendered.contains("wJalrXUtnFEMI"));
    assert!(!rendered.contains("sess-1"));
}
