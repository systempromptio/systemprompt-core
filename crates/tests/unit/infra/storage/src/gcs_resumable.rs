use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use bytes::Bytes;
use systemprompt_storage::{
    FileStorageBackend, GcsError, GcsParams, GcsTokenSource, build_file_storage,
};
use systemprompt_traits::{FileByteStream, FileStorage, FileStorageError};
use url::Url;
use wiremock::matchers::{body_bytes, header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const CHUNK: usize = 8 * 1024 * 1024;
const SESSION: &str = "/session/s-1";

struct StaticTokens;

#[async_trait]
impl GcsTokenSource for StaticTokens {
    async fn bearer(&self) -> Result<String, GcsError> {
        Ok("tok-1".to_owned())
    }
}

fn storage(server: &MockServer) -> Arc<dyn FileStorage> {
    let base = Url::parse(&server.uri()).unwrap();
    let params = GcsParams::new("my-bucket", None, false)
        .unwrap()
        .with_endpoints(base.clone(), base);
    build_file_storage(FileStorageBackend::Gcs {
        params: Box::new(params),
        tokens: Arc::new(StaticTokens),
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

async fn mount_open(server: &MockServer, location: Option<String>, status: u16) {
    let mut response = ResponseTemplate::new(status);
    if let Some(location) = location {
        response = response.insert_header("location", location.as_str());
    }
    Mock::given(method("POST"))
        .and(path("/upload/storage/v1/b/my-bucket/o"))
        .and(query_param("uploadType", "resumable"))
        .and(query_param("name", "big.bin"))
        .and(header("authorization", "Bearer tok-1"))
        .and(header("x-upload-content-type", "application/octet-stream"))
        .respond_with(response)
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_chunk(server: &MockServer, range: &str, status: u16) {
    Mock::given(method("PUT"))
        .and(path(SESSION))
        .and(header("content-range", range))
        .respond_with(ResponseTemplate::new(status).set_body_string("chunk-answer"))
        .expect(1)
        .mount(server)
        .await;
}

async fn mount_cancel(server: &MockServer, times: u64) {
    Mock::given(method("DELETE"))
        .and(path(SESSION))
        .respond_with(ResponseTemplate::new(499))
        .expect(times)
        .mount(server)
        .await;
}

fn session(server: &MockServer) -> Option<String> {
    Some(format!("{}{SESSION}", server.uri()))
}

#[tokio::test]
async fn a_short_stream_is_one_media_upload() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/upload/storage/v1/b/my-bucket/o"))
        .and(query_param("uploadType", "media"))
        .and(body_bytes(b"small".to_vec()))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&server)
        .await;
    let id = storage(&server)
        .store_stream(
            Path::new("big.bin"),
            stream_of(vec![Ok(Bytes::from_static(b"small"))]),
        )
        .await
        .unwrap();
    assert_eq!(id.as_str(), "big.bin");
}

#[tokio::test]
async fn an_empty_stream_is_an_empty_media_upload() {
    let server = MockServer::start().await;
    Mock::given(method("POST"))
        .and(path("/upload/storage/v1/b/my-bucket/o"))
        .and(query_param("uploadType", "media"))
        .and(body_bytes(Vec::<u8>::new()))
        .respond_with(ResponseTemplate::new(200).set_body_string("{}"))
        .expect(1)
        .mount(&server)
        .await;
    storage(&server)
        .store_stream(Path::new("big.bin"), stream_of(vec![]))
        .await
        .unwrap();
}

#[tokio::test]
async fn a_long_stream_sends_ranged_chunks_to_the_session() {
    let server = MockServer::start().await;
    mount_open(&server, session(&server), 200).await;
    mount_chunk(&server, &format!("bytes 0-{}/*", CHUNK - 1), 308).await;
    mount_chunk(
        &server,
        &format!("bytes {CHUNK}-{}/{}", CHUNK + 3, CHUNK + 4),
        200,
    )
    .await;
    mount_cancel(&server, 0).await;
    let id = storage(&server)
        .store_stream(Path::new("big.bin"), two_chunk_body())
        .await
        .unwrap();
    assert_eq!(id.as_str(), "big.bin");
}

#[tokio::test]
async fn a_refused_session_open_reports_the_status() {
    let server = MockServer::start().await;
    mount_open(&server, None, 403).await;
    let error = storage(&server)
        .store_stream(Path::new("big.bin"), two_chunk_body())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("403"), "{error}");
}

#[tokio::test]
async fn a_session_without_a_location_is_refused() {
    let server = MockServer::start().await;
    mount_open(&server, None, 200).await;
    let error = storage(&server)
        .store_stream(Path::new("big.bin"), two_chunk_body())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("location"), "{error}");
}

#[tokio::test]
async fn a_chunk_not_answered_308_cancels_the_session() {
    let server = MockServer::start().await;
    mount_open(&server, session(&server), 200).await;
    mount_chunk(&server, &format!("bytes 0-{}/*", CHUNK - 1), 200).await;
    mount_cancel(&server, 1).await;
    let error = storage(&server)
        .store_stream(Path::new("big.bin"), two_chunk_body())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("200"), "{error}");
}

#[tokio::test]
async fn a_failed_last_chunk_cancels_the_session() {
    let server = MockServer::start().await;
    mount_open(&server, session(&server), 200).await;
    mount_chunk(&server, &format!("bytes 0-{}/*", CHUNK - 1), 308).await;
    mount_chunk(
        &server,
        &format!("bytes {CHUNK}-{}/{}", CHUNK + 3, CHUNK + 4),
        500,
    )
    .await;
    mount_cancel(&server, 1).await;
    let error = storage(&server)
        .store_stream(Path::new("big.bin"), two_chunk_body())
        .await
        .unwrap_err();
    assert!(error.to_string().contains("500"), "{error}");
}

#[tokio::test]
async fn a_source_error_mid_stream_cancels_and_surfaces_it() {
    let server = MockServer::start().await;
    mount_open(&server, session(&server), 200).await;
    mount_chunk(&server, &format!("bytes 0-{}/*", CHUNK - 1), 308).await;
    mount_cancel(&server, 1).await;
    let body = stream_of(vec![
        Ok(Bytes::from(vec![b'a'; CHUNK])),
        Ok(Bytes::from(vec![b'b'; CHUNK])),
        Err(FileStorageError::Validation("broken source".to_owned())),
    ]);
    let error = storage(&server)
        .store_stream(Path::new("big.bin"), body)
        .await
        .unwrap_err();
    assert!(
        matches!(error, FileStorageError::Validation(_)),
        "{error:?}"
    );
}
