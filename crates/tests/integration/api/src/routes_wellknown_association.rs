//! `/.well-known/apple-app-site-association` and
//! `/.well-known/assetlinks.json` on an instance that vouches for an app of
//! each vendor.
//!
//! The documents come from configuration alone, so the same bytes answer a
//! bare request and one carrying a query string, a foreign `Host` and a
//! credential. The profile is isolated because the shared fixture has no
//! `gateway:` section, on which both routes answer 404.

use std::sync::OnceLock;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use serde_json::json;
use systemprompt_api::routes::wellknown_router;
use systemprompt_test_fixtures::{TestBootstrap, init_services_bootstrap, test_app_context_on};
use tower::ServiceExt;

use super::common::{body_to_string, empty_get};

static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

fn fingerprint() -> String {
    ["AB"; 32].join(":")
}

fn services_yaml() -> String {
    format!(
        r#"
credentials:
  anthropic:
    kind: platform
    secret: anthropic_api_key
providers:
  anthropic:
    credential: anthropic
    endpoints:
      api:
        wire: anthropic
        url: http://127.0.0.1:1
models:
  claude-fixture-1:
    clients:
    - claude
    pricing:
      input_per_million: 3.0
      output_per_million: 15.0
      cache_read_per_million: 0.0
deployments:
  claude-fixture-1:
  - anthropic/api
gateway:
  enabled: true
  bridge_releases:
    repo: systempromptio/bridge
    assets:
      android: example-android.apk
  bridge_instance:
    device_link_path: /bridge-auth/device-link
    identity_path: /api/public/bridge/whoami
    clients:
      ios:
        app_id: io.example.app
        min_version: 1.2.0
        link: https://apps.example.com/redeem
      android:
        app_id: io.example.android
        min_version: 1.3.0
    vouched_apps:
      ios:
      - team_id: ABCDE12345
        bundle_id: io.example.app
        link_paths:
        - /app/*
      android:
      - package_name: io.example.android
        sha256_cert_fingerprints:
        - "{fingerprint}"
"#,
        fingerprint = fingerprint()
    )
}

async fn app() -> anyhow::Result<Router> {
    let boot = BOOT.get_or_init(|| init_services_bootstrap(&services_yaml()));
    let ctx = test_app_context_on(boot).await;
    Ok(wellknown_router(&ctx)?)
}

fn apple_document() -> serde_json::Value {
    json!({
        "applinks": {
            "details": [
                {
                    "appIDs": ["ABCDE12345.io.example.app"],
                    "components": [{ "/": "/app/*" }],
                },
            ],
        },
        "webcredentials": { "apps": ["ABCDE12345.io.example.app"] },
    })
}

fn android_document() -> serde_json::Value {
    json!([
        {
            "relation": [
                "delegate_permission/common.handle_all_urls",
                "delegate_permission/common.get_login_creds",
            ],
            "target": {
                "namespace": "android_app",
                "package_name": "io.example.android",
                "sha256_cert_fingerprints": [fingerprint()],
            },
        },
    ])
}

fn hostile_get(uri: &str) -> Request<Body> {
    Request::builder()
        .uri(uri)
        .header(header::HOST, "evil.example")
        .header("x-forwarded-host", "evil.example")
        .header(header::AUTHORIZATION, "Bearer not-a-jwt")
        .header(header::ACCEPT, "text/html")
        .body(Body::empty())
        .expect("request build")
}

async fn served(request: Request<Body>) -> anyhow::Result<serde_json::Value> {
    let response = app().await?.oneshot(request).await?;
    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_TYPE)
            .and_then(|v| v.to_str().ok()),
        Some("application/json")
    );
    assert_eq!(
        response
            .headers()
            .get(header::CACHE_CONTROL)
            .and_then(|v| v.to_str().ok()),
        Some("public, max-age=3600")
    );
    let (_, body) = body_to_string(response).await?;
    Ok(serde_json::from_str(&body)?)
}

#[tokio::test]
async fn the_apple_document_is_served_as_cacheable_json_without_a_credential() -> anyhow::Result<()>
{
    let document = served(empty_get("/.well-known/apple-app-site-association")).await?;
    assert_eq!(document, apple_document());
    Ok(())
}

#[tokio::test]
async fn the_asset_links_document_is_served_as_cacheable_json_without_a_credential()
-> anyhow::Result<()> {
    let document = served(empty_get("/.well-known/assetlinks.json")).await?;
    assert_eq!(document, android_document());
    Ok(())
}

#[tokio::test]
async fn request_input_does_not_reach_either_document() -> anyhow::Result<()> {
    let apple = served(hostile_get(
        "/.well-known/apple-app-site-association?apps=EVIL000000.io.evil.app",
    ))
    .await?;
    assert_eq!(apple, apple_document());

    let android = served(hostile_get(
        "/.well-known/assetlinks.json?package_name=io.evil.app",
    ))
    .await?;
    assert_eq!(android, android_document());
    Ok(())
}

#[tokio::test]
async fn the_discovery_document_lists_the_configured_apps() -> anyhow::Result<()> {
    let response = app()
        .await?
        .oneshot(empty_get("/.well-known/systemprompt-bridge"))
        .await?;
    assert_eq!(response.status().as_u16(), 200);
    let (_, body) = body_to_string(response).await?;
    let document: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(
        document["clients"],
        json!({
            "ios": {
                "app_id": "io.example.app",
                "min_version": "1.2.0",
                "link": "https://apps.example.com/redeem",
            },
            "android": {
                "app_id": "io.example.android",
                "min_version": "1.3.0",
                "download_path": "/v1/bridge/download/android",
            },
        })
    );
    Ok(())
}
