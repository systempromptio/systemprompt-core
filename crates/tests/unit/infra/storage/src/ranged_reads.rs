use std::path::Path;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_storage::{
    GcsError, GcsFileStorage, GcsParams, GcsTokenSource, LocalFileStorage, S3Credentials,
    S3FileStorage, S3Params,
};
use systemprompt_traits::{ByteRange, FileStorage, FileStorageError, ServedRange, StoredFileId};
use tempfile::TempDir;
use url::Url;
use wiremock::matchers::{header, method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

use crate::support::ReadAll;

async fn local_with(content: &[u8]) -> (TempDir, LocalFileStorage, StoredFileId) {
    let dir = TempDir::new().unwrap();
    let storage = LocalFileStorage::new(dir.path().to_path_buf());
    let id = storage.store(Path::new("files/a.bin"), content).await.unwrap();
    (dir, storage, id)
}

#[tokio::test]
async fn local_open_streams_the_whole_file_with_size_and_etag() {
    let (_dir, s, id) = local_with(b"0123456789").await;
    let read = s.open(&id, None).await.unwrap();
    assert!(read.range.is_none());
    assert_eq!(read.metadata.size_bytes, Some(10));
    assert!(read.metadata.etag.is_some());
    assert_eq!(s.read_all(&id).await.unwrap(), b"0123456789");
}

#[tokio::test]
async fn local_ranges_serve_the_requested_bytes() {
    let (_dir, s, id) = local_with(b"0123456789").await;
    let cases = [
        (ByteRange::Bounded { start: 2, end: 4 }, &b"234"[..], (2, 4)),
        (ByteRange::From(7), &b"789"[..], (7, 9)),
        (ByteRange::Suffix(3), &b"789"[..], (7, 9)),
        (ByteRange::Bounded { start: 8, end: 100 }, &b"89"[..], (8, 9)),
        (ByteRange::Suffix(50), &b"0123456789"[..], (0, 9)),
    ];
    for (range, expected, (start, end)) in cases {
        let read = s.open(&id, Some(range)).await.unwrap();
        assert_eq!(
            read.range,
            Some(ServedRange {
                start,
                end,
                size: 10
            }),
            "{range:?}"
        );
        assert_eq!(s.read_range(&id, range).await.unwrap(), expected, "{range:?}");
    }
}

#[tokio::test]
async fn local_unsatisfiable_ranges_report_the_size() {
    let (_dir, s, id) = local_with(b"0123456789").await;
    for range in [ByteRange::From(10), ByteRange::Suffix(0)] {
        assert!(matches!(
            s.open(&id, Some(range)).await,
            Err(FileStorageError::RangeNotSatisfiable { size: 10 })
        ));
    }
}

#[tokio::test]
async fn local_etag_changes_when_the_file_changes() {
    let (_dir, s, id) = local_with(b"one").await;
    let first = s.metadata(&id).await.unwrap().etag;
    s.store(Path::new("files/a.bin"), b"three").await.unwrap();
    let second = s.metadata(&id).await.unwrap().etag;
    assert_ne!(first, second);
    assert_eq!(s.open(&id, None).await.unwrap().metadata.etag, second);
}

#[tokio::test]
async fn local_directories_are_not_found() {
    let (_dir, s, _id) = local_with(b"x").await;
    assert!(matches!(
        s.open(&StoredFileId::new("files"), None).await,
        Err(FileStorageError::NotFound(_))
    ));
}

fn s3(server: &MockServer) -> S3FileStorage {
    let credentials: S3Credentials = serde_json::from_value(serde_json::json!({
        "access_key_id": "AKID",
        "secret_access_key": "secret",
    }))
    .unwrap();
    S3FileStorage::new(
        S3Params::new("b", None, Some(&server.uri()))
            .unwrap()
            .with_path_style(true),
        credentials,
        reqwest::Client::new(),
    )
}

#[tokio::test]
async fn s3_forwards_the_range_and_reads_a_206() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/b/files/v.mp4"))
        .and(header("range", "bytes=2-4"))
        .respond_with(
            ResponseTemplate::new(206)
                .insert_header("content-range", "bytes 2-4/10")
                .insert_header("content-type", "video/mp4")
                .insert_header("etag", "\"abc\"")
                .set_body_bytes(b"234".to_vec()),
        )
        .expect(2)
        .mount(&server)
        .await;
    let s = s3(&server);
    let id = StoredFileId::new("files/v.mp4");
    let range = ByteRange::Bounded { start: 2, end: 4 };
    let read = s.open(&id, Some(range)).await.unwrap();
    assert_eq!(
        read.range,
        Some(ServedRange {
            start: 2,
            end: 4,
            size: 10
        })
    );
    assert_eq!(read.metadata.size_bytes, Some(10));
    assert_eq!(read.metadata.etag.as_deref(), Some("\"abc\""));
    assert_eq!(read.metadata.mime_type, "video/mp4");
    drop(read);
    assert_eq!(s.read_range(&id, range).await.unwrap(), b"234");
}

#[tokio::test]
async fn s3_a_416_is_range_not_satisfiable_with_the_size() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(416).insert_header("content-range", "bytes */10"))
        .mount(&server)
        .await;
    assert!(matches!(
        s3(&server)
            .open(&StoredFileId::new("files/v.mp4"), Some(ByteRange::From(50)))
            .await,
        Err(FileStorageError::RangeNotSatisfiable { size: 10 })
    ));
}

#[tokio::test]
async fn s3_a_range_the_server_ignores_is_served_whole() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"0123456789".to_vec()))
        .mount(&server)
        .await;
    let read = s3(&server)
        .open(&StoredFileId::new("files/a"), Some(ByteRange::From(5)))
        .await
        .unwrap();
    assert!(read.range.is_none());
    assert_eq!(read.metadata.size_bytes, Some(10));
}

struct StaticTokens;

#[async_trait]
impl GcsTokenSource for StaticTokens {
    async fn bearer(&self) -> Result<String, GcsError> {
        Ok("tok".to_owned())
    }
}

#[tokio::test]
async fn gcs_forwards_the_range_and_reads_a_206() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/storage/v1/b/b/o/files%2Fa.bin"))
        .and(query_param("alt", "media"))
        .and(header("range", "bytes=-3"))
        .respond_with(
            ResponseTemplate::new(206)
                .insert_header("content-range", "bytes 7-9/10")
                .set_body_bytes(b"789".to_vec()),
        )
        .expect(1)
        .mount(&server)
        .await;
    let base = Url::parse(&server.uri()).unwrap();
    let s = GcsFileStorage::new(
        GcsParams::new("b", None, false)
            .unwrap()
            .with_endpoints(base.clone(), base),
        Arc::new(StaticTokens),
        reqwest::Client::new(),
    );
    let read = s
        .open(&StoredFileId::new("files/a.bin"), Some(ByteRange::Suffix(3)))
        .await
        .unwrap();
    assert_eq!(
        read.range,
        Some(ServedRange {
            start: 7,
            end: 9,
            size: 10
        })
    );
}
