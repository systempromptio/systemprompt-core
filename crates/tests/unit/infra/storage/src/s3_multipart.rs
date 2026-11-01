use std::path::Path;
use std::sync::Arc;

use bytes::Bytes;
use systemprompt_storage::{FileStorageBackend, S3Credentials, S3Params, build_file_storage};
use systemprompt_traits::{FileByteStream, FileStorage, FileStorageError};
use wiremock::matchers::{body_bytes, body_string_contains, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CHUNK: usize = 8 * 1024 * 1024;
const OBJECT: &str = "/my-bucket/big.bin";
const CREATED: &str =
    "<InitiateMultipartUploadResult><UploadId>up-1</UploadId></InitiateMultipartUploadResult>";

fn storage(server: &MockServer) -> Arc<dyn FileStorage> {
    let credentials: S3Credentials = serde_json::from_value(serde_json::json!({
        "access_key_id": "AKIDEXAMPLE",
        "secret_access_key": "secret",
        "session_token": null,
    }))
    .unwrap();
    let params = S3Params::new(
        "my-bucket",
        Some("eu-west-1".to_owned()),
        Some(&server.uri()),
    )
    .unwrap()
    .with_path_style(true);
    build_file_storage(FileStorageBackend::S3 {
        params: Box::new(params),
        credentials,
        http: reqwest::Client::new(),
    })
}

fn stream_of(parts: Vec<Result<Bytes, FileStorageError>>) -> FileByteStream {
    Box::pin(futures::stream::iter(parts))
}

fn two_chunk_body() -> FileByteStream {
    stream_of(vec![
        Ok(Bytes::from(vec![b'a'; CHUNK])),
        Ok(Bytes::from_static(b"tail")),
    ])
}

async fn mount_create(server: &MockServer, body: &str) {
    Mock::given(method("POST"))
        .and(path(OBJECT))
        .and(query_param("uploads", ""))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_owned()))
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_part(server: &MockServer, number: &str, etag: Option<&str>) {
    let mut response = ResponseTemplate::new(200);
    if let Some(etag) = etag {
        response = response.insert_header("etag", etag);
    }
    Mock::given(method("PUT"))
        .and(path(OBJECT))
        .and(query_param("partNumber", number))
        .and(query_param("uploadId", "up-1"))
        .respond_with(response)
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_abort(server: &MockServer, times: u64) {
    Mock::given(method("DELETE"))
        .and(path(OBJECT))
        .and(query_param("uploadId", "up-1"))
        .respond_with(ResponseTemplate::new(204))
        .expect(times)
        .mount(server)
        .await;
}

#[tokio::test]
async fn an_empty_stream_is_one_empty_put() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path(OBJECT))
        .and(body_bytes(Vec::<u8>::new()))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    let id = storage(&server)
        .store_stream(Path::new("big.bin"), stream_of(vec![]))
        .await
        .unwrap();
    assert_eq!(id.as_str(), "big.bin");
}

#[tokio::test]
async fn a_stream_within_one_chunk_is_a_single_put() {
    let server = MockServer::start().await;
    Mock::given(method("PUT"))
        .and(path(OBJECT))
        .and(body_bytes(b"hello world".to_vec()))
        .respond_with(ResponseTemplate::new(200))
        .expect(1)
        .mount(&server)
        .await;
    let body = stream_of(vec![
        Ok(Bytes::from_static(b"hello ")),
        Ok(Bytes::from_static(b"world")),
    ]);
    storage(&server)
        .store_stream(Path::new("big.bin"), body)
        .await
        .unwrap();
}

#[tokio::test]
async fn a_long_stream_uploads_parts_and_completes_with_their_etags() {
    let server = MockServer::start().await;
    mount_create(&server, CREATED).await;
    mount_part(&server, "1", Some("\"e1\"")).await;
    mount_part(&server, "2", Some("\"e2\"")).await;
    Mock::given(method("POST"))
        .and(path(OBJECT))
        .and(query_param("uploadId", "up-1"))
        .and(body_string_contains(
            "<Part><PartNumber>1</PartNumber><ETag>\"e1\"</ETag></Part><Part><PartNumber>2</PartNumber><ETag>\"e2\"</ETag></Part>",
        ))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("<CompleteMultipartUploadResult/>"),
        )
        .expect(1)
        .mount(&server)
        .await;
    mount_abort(&server, 0).await;
    let id = storage(&server)
        .store_stream(Path::new("big.bin"), two_chunk_body())
        .await
        .unwrap();
    assert_eq!(id.as_str(), "big.bin");
}

#[tokio::test]
async fn a_create_without_an_upload_id_is_refused() {
    let server = MockServer::start().await;
    mount_create(&server, "<InitiateMultipartUploadResult/>").await;
    let error = storage(&server)
        .store_stream(Path::new("big.bin"), two_chunk_body())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("UploadId"), "{error}");
}

#[tokio::test]
async fn a_part_without_an_etag_aborts_the_upload() {
    let server = MockServer::start().await;
    mount_create(&server, CREATED).await;
    mount_part(&server, "1", None).await;
    mount_abort(&server, 1).await;
    let error = storage(&server)
        .store_stream(Path::new("big.bin"), two_chunk_body())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("etag"), "{error}");
}

#[tokio::test]
async fn a_complete_answered_200_with_an_error_body_aborts() {
    let server = MockServer::start().await;
    mount_create(&server, CREATED).await;
    mount_part(&server, "1", Some("\"e1\"")).await;
    mount_part(&server, "2", Some("\"e2\"")).await;
    Mock::given(method("POST"))
        .and(path(OBJECT))
        .and(query_param("uploadId", "up-1"))
        .respond_with(
            ResponseTemplate::new(200).set_body_string("<Error><Code>InternalError</Code></Error>"),
        )
        .expect(1)
        .mount(&server)
        .await;
    mount_abort(&server, 1).await;
    let error = storage(&server)
        .store_stream(Path::new("big.bin"), two_chunk_body())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("InternalError"), "{error}");
}

#[tokio::test]
async fn a_source_error_mid_stream_aborts_and_surfaces_the_source_error() {
    let server = MockServer::start().await;
    mount_create(&server, CREATED).await;
    mount_part(&server, "1", Some("\"e1\"")).await;
    mount_abort(&server, 1).await;
    let body = stream_of(vec![
        Ok(Bytes::from(vec![b'a'; CHUNK])),
        Ok(Bytes::from(vec![b'b'; CHUNK])),
        Err(FileStorageError::NotFound("source".to_owned())),
    ]);
    let error = storage(&server)
        .store_stream(Path::new("big.bin"), body)
        .await
        .unwrap_err();
    assert!(matches!(error, FileStorageError::NotFound(_)), "{error:?}");
}
