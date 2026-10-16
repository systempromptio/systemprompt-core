use std::path::Path;
use std::sync::Arc;

use axum::body::to_bytes;
use axum::extract::State;
use axum::http::{HeaderMap, StatusCode, Uri, header};
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
    let resp = serve_static_content(
        State(state.clone()),
        uri.parse::<Uri>().expect("uri"),
        HeaderMap::new(),
        None,
    )
    .await
    .into_response();
    let status = resp.status();
    let headers = resp.headers().clone();
    let body = to_bytes(resp.into_body(), 1024 * 1024).await.unwrap().to_vec();
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
    assert!(!local.exists(), "the fixture must not share the writable layer");
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
