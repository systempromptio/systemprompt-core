//! `GET /bridge/plugins/{id}/{*path}` — auth rejection, path-safety guard,
//! bundle lookup misses, and the content-type / path-safety helpers exposed
//! via `test-api`.

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use http::StatusCode;
use systemprompt_api::routes::gateway::bridge_data::load_services_config;
use systemprompt_api::routes::gateway::bridge_plugin_file::test_api::{
    content_type, relative_path_is_safe,
};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_database::DbPool;
use systemprompt_test_fixtures::{install_test_signing_key, seed_bridge_credential};
use tower::ServiceExt;

use super::common::setup_ctx;

async fn router_and_pool() -> anyhow::Result<(Router, DbPool)> {
    let (pool, ctx) = setup_ctx().await?;
    install_test_signing_key();
    Ok((gateway_router(&ctx).expect("gateway router"), pool))
}

fn authed_get(uri: &str, token: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::AUTHORIZATION, format!("Bearer {token}"))
        .body(Body::empty())
        .expect("request build")
}

#[tokio::test]
async fn plugin_file_without_credential_is_unauthorized() -> anyhow::Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(
            Request::builder()
                .uri("/bridge/plugins/some-plugin/README.md")
                .body(Body::empty())?,
        )
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn plugin_file_with_bad_token_is_unauthorized() -> anyhow::Result<()> {
    let (app, _pool) = router_and_pool().await?;
    let resp = app
        .oneshot(authed_get(
            "/bridge/plugins/some-plugin/README.md",
            "not-a-jwt",
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::UNAUTHORIZED);
    Ok(())
}

#[tokio::test]
async fn plugin_file_traversal_path_is_rejected() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_bridge_credential(&pool, "plugin-traversal@example.invalid").await?;
    let resp = app
        .oneshot(authed_get(
            "/bridge/plugins/some-plugin/a/../secret.txt",
            cred.jwt.as_str(),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::BAD_REQUEST);
    Ok(())
}

#[tokio::test]
async fn plugin_file_unknown_plugin_misses_bundle_or_reports_unavailable() -> anyhow::Result<()> {
    let (app, pool) = router_and_pool().await?;
    let cred = seed_bridge_credential(&pool, "plugin-miss@example.invalid").await?;
    let resp = app
        .oneshot(authed_get(
            "/bridge/plugins/no-such-plugin/SKILL.md",
            cred.jwt.as_str(),
        ))
        .await?;
    if load_services_config().is_ok() {
        assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    } else {
        assert_eq!(resp.status(), StatusCode::INTERNAL_SERVER_ERROR);
    }
    Ok(())
}

const SKILL_CONFIG: &str = "id: covskill\nname: Cov Skill\ndescription: Skill served through the \
                            plugin bundle.\nenabled: true\ntags: []\n";

const PLUGIN_CONFIG: &str = r#"plugins:
  cov-plugin:
    id: cov-plugin
    name: Cov Plugin
    description: Plugin for byte-serve coverage.
    version: "1.0.0"
    enabled: true
    author:
      name: Test
      email: test@example.invalid
    keywords: []
    license: MIT
    category: testing
    skills:
      source: explicit
      include: [covskill]
    agents:
      source: explicit
"#;

fn seed_plugin_tree() {
    let b = systemprompt_test_fixtures::ensure_test_bootstrap();
    let skills = b.services_path.join("skills/covskill");
    std::fs::create_dir_all(&skills).expect("mkdir skill");
    std::fs::write(skills.join("config.yaml"), SKILL_CONFIG).expect("write skill config");
    std::fs::write(skills.join("index.md"), "Cov skill instructions.\n").expect("write skill md");
    std::fs::write(b.services_path.join("config/config.yaml"), PLUGIN_CONFIG)
        .expect("write services config");
}

pub(crate) async fn bundle_router_and_pool() -> anyhow::Result<(Router, DbPool)> {
    let b = systemprompt_test_fixtures::ensure_test_bootstrap();
    seed_plugin_tree();
    let pool = systemprompt_test_fixtures::fixture_db_pool(&b.database_url).await?;
    let paths = systemprompt_models::profile::PathsConfig {
        system: b.system_path.to_string_lossy().into_owned(),
        services: b.services_path.to_string_lossy().into_owned(),
        bin: b.bin_path.to_string_lossy().into_owned(),
        web_path: None,
        storage: Some(b.storage_path.to_string_lossy().into_owned()),
        geoip_database: None,
    };
    let ctx = systemprompt_test_fixtures::fixture_app_context_with(
        &pool,
        &b.database_url,
        paths,
        std::sync::Arc::new(systemprompt_marketplace::AllowAllFilter),
    )?;
    install_test_signing_key();
    Ok((gateway_router(&ctx).expect("gateway router"), pool))
}

#[tokio::test]
async fn plugin_file_serves_skill_bytes_with_markdown_content_type() -> anyhow::Result<()> {
    let (app, pool) = bundle_router_and_pool().await?;
    let cred = seed_bridge_credential(&pool, "plugin-serve@example.invalid").await?;
    let resp = app
        .oneshot(authed_get(
            "/bridge/plugins/cov-plugin/skills/covskill/SKILL.md",
            cred.jwt.as_str(),
        ))
        .await?;
    let status = resp.status();
    let ct = resp
        .headers()
        .get(header::CONTENT_TYPE)
        .and_then(|v| v.to_str().ok())
        .map(str::to_owned);
    if status != StatusCode::OK {
        let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await?;
        panic!("{status}: {}", String::from_utf8_lossy(&bytes));
    }
    assert_eq!(ct.as_deref(), Some("text/markdown; charset=utf-8"));
    let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await?;
    let body = String::from_utf8(bytes.to_vec())?;
    assert!(body.contains("name: covskill"), "{body}");
    assert!(body.contains("Cov skill instructions."), "{body}");
    Ok(())
}

#[tokio::test]
async fn plugin_manifest_serves_json_with_plugin_identity() -> anyhow::Result<()> {
    let (app, pool) = bundle_router_and_pool().await?;
    let cred = seed_bridge_credential(&pool, "plugin-manifest@example.invalid").await?;
    let resp = app
        .oneshot(authed_get(
            "/bridge/plugins/cov-plugin/.claude-plugin/plugin.json",
            cred.jwt.as_str(),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::OK);
    assert_eq!(
        resp.headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/json")
    );
    let bytes = axum::body::to_bytes(resp.into_body(), 1024 * 1024).await?;
    let manifest: serde_json::Value = serde_json::from_slice(&bytes)?;
    assert_eq!(manifest["name"].as_str(), Some("cov-plugin"));
    assert!(
        manifest["version"]
            .as_str()
            .is_some_and(|v| v.starts_with("1.0.0+")),
        "content-addressed version expected; got {manifest}"
    );
    Ok(())
}

#[tokio::test]
async fn plugin_file_unknown_file_in_known_plugin_returns_404() -> anyhow::Result<()> {
    let (app, pool) = bundle_router_and_pool().await?;
    let cred = seed_bridge_credential(&pool, "plugin-file-miss@example.invalid").await?;
    let resp = app
        .oneshot(authed_get(
            "/bridge/plugins/cov-plugin/skills/covskill/NOPE.md",
            cred.jwt.as_str(),
        ))
        .await?;
    assert_eq!(resp.status(), StatusCode::NOT_FOUND);
    Ok(())
}

#[test]
fn relative_path_safety_covers_traversal_and_absolute_forms() {
    assert!(relative_path_is_safe("SKILL.md"));
    assert!(relative_path_is_safe("dir/nested/file.json"));
    assert!(relative_path_is_safe("./file.txt"));
    assert!(!relative_path_is_safe(""));
    assert!(!relative_path_is_safe("../escape.md"));
    assert!(!relative_path_is_safe("dir/../../escape.md"));
    assert!(!relative_path_is_safe("/etc/passwd"));
}

#[test]
fn content_type_maps_known_extensions_and_defaults_to_octet_stream() {
    assert_eq!(content_type("a.md"), "text/markdown; charset=utf-8");
    assert_eq!(content_type("a.txt"), "text/plain; charset=utf-8");
    assert_eq!(content_type("a.json"), "application/json");
    assert_eq!(content_type("a.yaml"), "application/yaml");
    assert_eq!(content_type("a.yml"), "application/yaml");
    assert_eq!(content_type("a.toml"), "application/toml");
    assert_eq!(content_type("a.html"), "text/html; charset=utf-8");
    assert_eq!(content_type("a.HTM"), "text/html; charset=utf-8");
    assert_eq!(content_type("a.css"), "text/css; charset=utf-8");
    assert_eq!(content_type("a.js"), "application/javascript");
    assert_eq!(content_type("a.png"), "image/png");
    assert_eq!(content_type("a.jpg"), "image/jpeg");
    assert_eq!(content_type("a.JPEG"), "image/jpeg");
    assert_eq!(content_type("a.svg"), "image/svg+xml");
    assert_eq!(content_type("a.wasm"), "application/wasm");
    assert_eq!(content_type("a.bin"), "application/octet-stream");
    assert_eq!(content_type("no-extension"), "application/octet-stream");
}
