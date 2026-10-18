use std::path::Path;
use std::sync::Arc;

use axum::body::to_bytes;
use axum::extract::State;
use axum::http::{HeaderMap, Method, StatusCode, Uri, header};
use axum::response::IntoResponse;
use systemprompt_api::services::static_content::config::StaticContentMatcher;
use systemprompt_api::services::static_content::static_files::{
    StaticContentState, serve_static_content,
};
use systemprompt_files::FilesConfig;
use systemprompt_identifiers::FileId;
use systemprompt_manifest::profile::PathsConfig;
use systemprompt_marketplace::AllowAllFilter;
use systemprompt_models::RouteClassifier;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context_with, test_db_pool};
use tempfile::TempDir;

struct Layers {
    _tmp: TempDir,
    shipped: std::path::PathBuf,
    state: StaticContentState,
}

async fn layers() -> anyhow::Result<Layers> {
    let b = ensure_test_bootstrap();
    let pool = test_db_pool().await;
    let tmp = TempDir::new()?;
    let web = tmp.path().join("web");
    let storage = tmp.path().join("storage");
    let shipped = tmp.path().join("shipped");
    std::fs::create_dir_all(web.join("dist"))?;
    std::fs::create_dir_all(&storage)?;
    std::fs::create_dir_all(&shipped)?;
    let paths = PathsConfig {
        system: tmp.path().to_string_lossy().into_owned(),
        services: "/tmp".to_owned(),
        bin: "/tmp".to_owned(),
        web_path: Some(web.to_string_lossy().into_owned()),
        storage: Some(storage.to_string_lossy().into_owned()),
        geoip_database: None,
        shipped_files: Some(shipped.to_string_lossy().into_owned()),
        data: None,
    };
    let ctx = fixture_app_context_with(&pool, &b.database_url, paths, Arc::new(AllowAllFilter))?;
    let _ = FilesConfig::init(ctx.app_paths());
    Ok(Layers {
        _tmp: tmp,
        shipped,
        state: StaticContentState {
            ctx,
            matcher: Arc::new(StaticContentMatcher::empty()),
            route_classifier: Arc::new(RouteClassifier::new(None)),
        },
    })
}

fn files_url(relative: &str) -> String {
    format!("{}/{relative}", FilesConfig::get().unwrap().url_prefix())
}

async fn get(state: &StaticContentState, uri: &str) -> (StatusCode, HeaderMap, Vec<u8>) {
    request(state, Method::GET, uri, &[]).await
}

async fn request(
    state: &StaticContentState,
    method: Method,
    uri: &str,
    extra: &[(header::HeaderName, &str)],
) -> (StatusCode, HeaderMap, Vec<u8>) {
    let mut headers = HeaderMap::new();
    for (name, value) in extra {
        headers.insert(name.clone(), value.parse().unwrap());
    }
    let resp = serve_static_content(
        State(state.clone()),
        uri.parse::<Uri>().expect("uri"),
        method,
        headers,
        None,
    )
    .await
    .into_response();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = to_bytes(resp.into_body(), 1024 * 1024)
        .await
        .unwrap()
        .to_vec();
    (status, headers, body)
}

fn write(path: &Path, body: &str) {
    std::fs::create_dir_all(path.parent().unwrap()).unwrap();
    std::fs::write(path, body).unwrap();
}

#[tokio::test]
async fn a_file_only_in_the_shipped_layer_is_served() -> anyhow::Result<()> {
    let l = layers().await?;
    let rel = format!("css/shipped-{}.css", FileId::generate().as_str());
    write(&l.shipped.join(&rel), "body{}");
    let (status, headers, body) = get(&l.state, &files_url(&rel)).await;
    assert_eq!(status, StatusCode::OK);
    assert!(
        headers[header::CONTENT_TYPE]
            .to_str()?
            .starts_with("text/css")
    );
    assert_eq!(body, b"body{}");
    Ok(())
}

#[tokio::test]
async fn the_writable_layer_wins_over_the_shipped_layer() -> anyhow::Result<()> {
    let l = layers().await?;
    let rel = format!("css/override-{}.css", FileId::generate().as_str());
    let writable = FilesConfig::get().unwrap().files().join(&rel);
    write(&writable, "operator");
    write(&l.shipped.join(&rel), "image");
    let (status, _, body) = get(&l.state, &files_url(&rel)).await;
    std::fs::remove_file(&writable)?;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(body, b"operator");
    Ok(())
}

#[tokio::test]
async fn a_file_only_in_the_storage_backend_is_served_from_it() -> anyhow::Result<()> {
    let l = layers().await?;
    let rel = format!("uploads/backend-{}.png", FileId::generate().as_str());
    l.state
        .ctx
        .file_storage()
        .store(Path::new(&format!("files/{rel}")), b"png-bytes")
        .await?;
    let local = FilesConfig::get().unwrap().files().join(&rel);
    assert!(
        !local.exists(),
        "the fixture must not share the writable layer"
    );
    let (status, headers, body) = get(&l.state, &files_url(&rel)).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_TYPE], "image/png");
    assert!(headers.contains_key(header::ETAG));
    assert_eq!(body, b"png-bytes");
    Ok(())
}

#[tokio::test]
async fn a_file_in_no_layer_is_not_found() -> anyhow::Result<()> {
    let l = layers().await?;
    let rel = format!("uploads/absent-{}.png", FileId::generate().as_str());
    let (status, _, _) = get(&l.state, &files_url(&rel)).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn a_traversal_attempt_through_the_backend_is_not_found() -> anyhow::Result<()> {
    let l = layers().await?;
    let (status, _, _) = get(&l.state, &files_url("../../etc/passwd")).await;
    assert_eq!(status, StatusCode::NOT_FOUND);
    Ok(())
}

#[tokio::test]
async fn no_traversal_spelling_reaches_a_file_outside_the_layers() -> anyhow::Result<()> {
    let l = layers().await?;
    let root = l.shipped.parent().expect("tempdir root").to_path_buf();
    let name = format!("secret-{}.txt", FileId::generate().as_str());
    for dir in ["", "storage", "storage/files", "web", "shipped"] {
        write(&root.join(dir).join(&name), "SENTINEL");
    }
    for depth in 1..=5 {
        for (up, sep) in [
            ("..", "/"),
            ("%2e%2e", "/"),
            ("%2E%2E", "%2F"),
            ("..", "%2f"),
        ] {
            let mut rel = [up, sep].concat().repeat(depth);
            rel.push_str(&name);
            let (status, _, body) = get(&l.state, &files_url(&rel)).await;
            assert_ne!(status, StatusCode::OK, "{rel} was served");
            assert!(!body.windows(8).any(|w| w == b"SENTINEL"), "{rel} leaked");
        }
    }
    Ok(())
}

async fn backend_file(l: &Layers, body: &[u8]) -> String {
    let rel = format!("uploads/range-{}.mp4", FileId::generate().as_str());
    l.state
        .ctx
        .file_storage()
        .store(Path::new(&format!("files/{rel}")), body)
        .await
        .unwrap();
    files_url(&rel)
}

#[tokio::test]
async fn a_full_get_advertises_ranges_and_the_length() -> anyhow::Result<()> {
    let l = layers().await?;
    let url = backend_file(&l, b"0123456789").await;
    let (status, headers, body) = get(&l.state, &url).await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::ACCEPT_RANGES], "bytes");
    assert_eq!(headers[header::CONTENT_LENGTH], "10");
    assert_eq!(headers[header::CONTENT_TYPE], "video/mp4");
    assert!(headers.contains_key(header::LAST_MODIFIED));
    assert_eq!(body, b"0123456789");
    Ok(())
}

#[tokio::test]
async fn a_range_request_is_answered_with_206() -> anyhow::Result<()> {
    let l = layers().await?;
    let url = backend_file(&l, b"0123456789").await;
    let (status, headers, body) =
        request(&l.state, Method::GET, &url, &[(header::RANGE, "bytes=3-5")]).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(headers[header::CONTENT_RANGE], "bytes 3-5/10");
    assert_eq!(headers[header::CONTENT_LENGTH], "3");
    assert_eq!(body, b"345");
    let (status, _, body) =
        request(&l.state, Method::GET, &url, &[(header::RANGE, "bytes=-2")]).await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body, b"89");
    Ok(())
}

#[tokio::test]
async fn an_unsatisfiable_range_is_416_with_the_size() -> anyhow::Result<()> {
    let l = layers().await?;
    let url = backend_file(&l, b"0123456789").await;
    let (status, headers, body) =
        request(&l.state, Method::GET, &url, &[(header::RANGE, "bytes=20-")]).await;
    assert_eq!(status, StatusCode::RANGE_NOT_SATISFIABLE);
    assert_eq!(headers[header::CONTENT_RANGE], "bytes */10");
    assert!(body.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_malformed_or_multi_range_is_served_whole() -> anyhow::Result<()> {
    let l = layers().await?;
    let url = backend_file(&l, b"0123456789").await;
    for raw in ["bytes=0-1,4-5", "bytes=9-2", "lines=1-2"] {
        let (status, _, body) = request(&l.state, Method::GET, &url, &[(header::RANGE, raw)]).await;
        assert_eq!(status, StatusCode::OK, "{raw}");
        assert_eq!(body, b"0123456789", "{raw}");
    }
    Ok(())
}

#[tokio::test]
async fn head_returns_headers_without_a_body() -> anyhow::Result<()> {
    let l = layers().await?;
    let url = backend_file(&l, b"0123456789").await;
    let (status, headers, body) = request(
        &l.state,
        Method::HEAD,
        &url,
        &[(header::RANGE, "bytes=0-1")],
    )
    .await;
    assert_eq!(status, StatusCode::OK);
    assert_eq!(headers[header::CONTENT_LENGTH], "10");
    assert!(headers.contains_key(header::ETAG));
    assert!(body.is_empty());
    Ok(())
}

#[tokio::test]
async fn a_matching_etag_is_304() -> anyhow::Result<()> {
    let l = layers().await?;
    let url = backend_file(&l, b"0123456789").await;
    let (_, headers, _) = get(&l.state, &url).await;
    let etag = headers[header::ETAG].to_str()?.to_owned();
    let (status, _, body) = request(
        &l.state,
        Method::GET,
        &url,
        &[(header::IF_NONE_MATCH, &etag)],
    )
    .await;
    assert_eq!(status, StatusCode::NOT_MODIFIED);
    assert!(body.is_empty());
    Ok(())
}

#[tokio::test]
async fn shipped_layer_files_honour_ranges_too() -> anyhow::Result<()> {
    let l = layers().await?;
    let rel = format!("downloads/bin-{}.zip", FileId::generate().as_str());
    write(&l.shipped.join(&rel), "abcdefgh");
    let (status, headers, body) = request(
        &l.state,
        Method::GET,
        &files_url(&rel),
        &[(header::RANGE, "bytes=4-")],
    )
    .await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(headers[header::CONTENT_RANGE], "bytes 4-7/8");
    assert_eq!(body, b"efgh");
    Ok(())
}

#[tokio::test]
async fn a_weak_or_listed_if_none_match_is_304() -> anyhow::Result<()> {
    let l = layers().await?;
    let url = backend_file(&l, b"0123456789").await;
    let (_, headers, _) = get(&l.state, &url).await;
    let etag = headers[header::ETAG].to_str()?.to_owned();
    for candidate in [
        format!("W/{etag}"),
        format!("\"other\", {etag}"),
        "*".to_owned(),
    ] {
        let (status, _, _) = request(
            &l.state,
            Method::GET,
            &url,
            &[(header::IF_NONE_MATCH, &candidate)],
        )
        .await;
        assert_eq!(status, StatusCode::NOT_MODIFIED, "{candidate}");
    }
    Ok(())
}

#[tokio::test]
async fn a_stale_if_range_is_served_whole() -> anyhow::Result<()> {
    let l = layers().await?;
    let url = backend_file(&l, b"0123456789").await;
    let (_, headers, _) = get(&l.state, &url).await;
    let etag = headers[header::ETAG].to_str()?.to_owned();
    let (status, _, body) = request(
        &l.state,
        Method::GET,
        &url,
        &[(header::RANGE, "bytes=3-5"), (header::IF_RANGE, &etag)],
    )
    .await;
    assert_eq!(status, StatusCode::PARTIAL_CONTENT);
    assert_eq!(body, b"345");
    for range in ["bytes=3-5", "bytes=20-"] {
        let (status, headers, body) = request(
            &l.state,
            Method::GET,
            &url,
            &[(header::RANGE, range), (header::IF_RANGE, "\"stale\"")],
        )
        .await;
        assert_eq!(status, StatusCode::OK, "{range}");
        assert!(!headers.contains_key(header::CONTENT_RANGE), "{range}");
        assert_eq!(body, b"0123456789", "{range}");
    }
    Ok(())
}
