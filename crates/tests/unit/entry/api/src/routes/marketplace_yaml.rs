//! `GET /marketplaces/{id}/manifest.yaml` streams a marketplace's raw
//! `config.yaml` from under the services root.
//!
//! The containment check on the canonicalised path is the whole boundary: an
//! id whose directory resolves outside the marketplaces root must be refused
//! outright rather than streamed, and it must be refused as forbidden, not as
//! a miss — a 404 there would make an escape indistinguishable from a typo.

use axum::body::Body;
use axum::http::{Request, StatusCode};
use std::path::PathBuf;
use systemprompt_runtime::AppContext;
use systemprompt_test_fixtures::{ensure_test_bootstrap, fixture_app_context, fixture_db_pool};
use tower::ServiceExt;

struct Scratch {
    root: PathBuf,
    id: String,
    escape_target: PathBuf,
}

impl Scratch {
    fn new() -> Self {
        let id = format!("cov-{}", uuid::Uuid::new_v4());
        let root = PathBuf::from("/tmp/marketplaces");
        std::fs::create_dir_all(&root).expect("marketplaces root");
        let escape_target = PathBuf::from(format!("/tmp/{id}-outside"));
        Self {
            root,
            id,
            escape_target,
        }
    }

    fn entry(&self) -> PathBuf {
        self.root.join(&self.id)
    }
}

impl Drop for Scratch {
    fn drop(&mut self) {
        let _ = std::fs::remove_file(self.entry());
        let _ = std::fs::remove_dir_all(self.entry());
        let _ = std::fs::remove_dir_all(&self.escape_target);
    }
}

async fn context() -> AppContext {
    let boot = ensure_test_bootstrap();
    let pool = fixture_db_pool(&boot.database_url)
        .await
        .expect("test database");
    (*fixture_app_context(&pool, &boot.database_url).expect("fixture context")).clone()
}

async fn fetch(id: &str) -> (StatusCode, Vec<u8>) {
    let ctx = context().await;
    let response = systemprompt_api::routes::marketplace::router()
        .with_state(ctx)
        .oneshot(
            Request::builder()
                .uri(format!("/marketplaces/{id}/manifest.yaml"))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router response");
    let status = response.status();
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("body");
    (status, body.to_vec())
}

#[tokio::test]
async fn a_marketplace_with_no_directory_is_not_found() {
    let scratch = Scratch::new();

    let (status, _) = fetch(&scratch.id).await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "an id that names no directory has no manifest to serve"
    );
}

#[tokio::test]
async fn a_marketplace_directory_without_a_config_is_not_found() {
    let scratch = Scratch::new();
    std::fs::create_dir_all(scratch.entry()).expect("marketplace directory");

    let (status, _) = fetch(&scratch.id).await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "a marketplace directory that carries no config.yaml is a miss, not a server fault"
    );
}

#[tokio::test]
async fn a_config_path_that_is_a_directory_is_not_served() {
    let scratch = Scratch::new();
    std::fs::create_dir_all(scratch.entry().join("config.yaml")).expect("config.yaml directory");

    let (status, _) = fetch(&scratch.id).await;

    assert_eq!(
        status,
        StatusCode::NOT_FOUND,
        "config.yaml must be a file before any read is attempted"
    );
}

#[cfg(unix)]
#[tokio::test]
async fn a_marketplace_entry_that_escapes_the_root_is_forbidden() {
    let scratch = Scratch::new();
    std::fs::create_dir_all(&scratch.escape_target).expect("escape target");
    std::fs::write(scratch.escape_target.join("config.yaml"), b"name: stolen\n")
        .expect("planted config");
    std::os::unix::fs::symlink(&scratch.escape_target, scratch.entry()).expect("escaping symlink");

    let (status, body) = fetch(&scratch.id).await;

    assert_eq!(
        status,
        StatusCode::FORBIDDEN,
        "a marketplace id that resolves outside the marketplaces root is a rejected request, not \
         a missing one"
    );
    assert!(
        !body.windows(6).any(|w| w == b"stolen"),
        "the escaped file's contents must never reach the caller"
    );
}

#[tokio::test]
async fn a_marketplace_config_is_served_verbatim_as_yaml() {
    let scratch = Scratch::new();
    std::fs::create_dir_all(scratch.entry()).expect("marketplace directory");
    std::fs::write(scratch.entry().join("config.yaml"), b"name: coverage\n").expect("config");

    let ctx = context().await;
    let response = systemprompt_api::routes::marketplace::router()
        .with_state(ctx)
        .oneshot(
            Request::builder()
                .uri(format!("/marketplaces/{}/manifest.yaml", scratch.id))
                .body(Body::empty())
                .expect("request"),
        )
        .await
        .expect("router response");

    assert_eq!(response.status(), StatusCode::OK);
    assert_eq!(
        response
            .headers()
            .get(axum::http::header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/yaml")
    );
    let body = axum::body::to_bytes(response.into_body(), 1 << 20)
        .await
        .expect("body");
    assert_eq!(
        body.as_ref(),
        b"name: coverage\n",
        "the manifest is streamed byte-for-byte, not re-serialised"
    );
}
