//! `/bridge/latest` and `/bridge/download/{platform}` driven
//! against a stub GitHub.
//!
//! These routes decide which binary an installed bridge updates itself to, so
//! the interesting behaviour is all in what they refuse: a tag that is not the
//! bridge's, a draft or prerelease, a platform with no published asset, an
//! upstream that answers badly. Every one of those needed a real call to
//! api.github.com to reach until `BridgeReleasesSpec` gained `api_base`;
//! they now run against wiremock.
//!
//! The profile is isolated per suite because the shared fixture has no
//! `gateway:` section at all, so these routes would otherwise short-circuit
//! before reaching any of it.

use std::sync::OnceLock;

use axum::Router;
use axum::body::Body;
use axum::http::{Request, header};
use systemprompt_api::routes::gateway::gateway_router;
use systemprompt_test_fixtures::{
    TestBootstrap, fixture_app_context, fixture_db_pool, init_services_bootstrap,
    install_test_signing_key, seed_admin_credential,
};
use tower::ServiceExt;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

use super::common::body_to_string;

static SERVER: OnceLock<MockServer> = OnceLock::new();
static BOOT: OnceLock<TestBootstrap> = OnceLock::new();

/// The stub is started once and its address baked into the profile, because
/// `init_services_bootstrap` writes the services config before any test runs.
async fn server() -> &'static MockServer {
    if let Some(s) = SERVER.get() {
        return s;
    }
    let s = MockServer::start().await;
    SERVER.get_or_init(|| s)
}

fn gateway_yaml(api_base: &str) -> String {
    format!(
        r#"
providers:
  - name: anthropic
    wire: anthropic
    surface: anthropic
    endpoint: http://127.0.0.1:1
    api_key_secret: anthropic_api_key
    models:
      - id: claude-fixture-1
        pricing:
          input_per_million: 3.0
          output_per_million: 15.0
gateway:
  enabled: true
  allow_unlisted_models: false
  routes:
    - id: claude
      model_pattern: "claude-*"
      provider: anthropic
  bridge_releases:
    repo: systempromptio/systemprompt-core
    tag_prefix: "bridge-v"
    api_base: "{api_base}"
    assets:
      darwin-arm64: systemprompt-bridge-darwin-arm64.tar.gz
"#
    )
}

async fn boot() -> &'static TestBootstrap {
    let base = server().await.uri();
    if let Some(b) = BOOT.get() {
        return b;
    }
    let b = init_services_bootstrap(&gateway_yaml(&base));
    BOOT.get_or_init(|| b)
}

async fn app() -> anyhow::Result<(Router, String)> {
    let b = boot().await;
    install_test_signing_key();
    let pool = fixture_db_pool(&b.database_url).await?;
    let fixture = seed_admin_credential(&pool, "bridge-release").await?;
    let ctx = fixture_app_context(&pool, &b.database_url)?;
    Ok((
        gateway_router(&ctx).expect("gateway router available"),
        fixture.jwt.as_str().to_owned(),
    ))
}

fn get(uri: &str, token: Option<&str>) -> Request<Body> {
    let mut builder = Request::builder().uri(uri);
    if let Some(token) = token {
        builder = builder.header(header::AUTHORIZATION, format!("Bearer {token}"));
    }
    builder.body(Body::empty()).expect("request must build")
}

const ASSET: &str = "systemprompt-bridge-darwin-arm64.tar.gz";
const DIGEST: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";

fn release(base: &str, tag: &str, draft: bool, prerelease: bool) -> serde_json::Value {
    serde_json::json!({
        "tag_name": tag,
        "draft": draft,
        "prerelease": prerelease,
        "html_url": format!("https://example.invalid/{tag}"),
        "assets": [
            { "name": ASSET, "url": format!("{base}/asset"), "size": 1234 },
            { "name": "SHA256SUMS", "url": format!("{base}/sums"), "size": 64 }
        ]
    })
}

async fn mount_releases(body: serde_json::Value) {
    let s = server().await;
    s.reset().await;
    Mock::given(method("GET"))
        .and(path("/repos/systempromptio/systemprompt-core/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(s)
        .await;
}

#[tokio::test]
async fn an_unauthenticated_caller_is_refused() -> anyhow::Result<()> {
    let (app, _token) = app().await?;

    let (status, _) = body_to_string(
        app.oneshot(get("/bridge/latest?platform=darwin-arm64", None))
            .await?,
    )
    .await?;

    assert_eq!(
        status.as_u16(),
        401,
        "the release endpoint must not serve an anonymous caller"
    );
    Ok(())
}

#[tokio::test]
async fn a_bad_token_is_refused() -> anyhow::Result<()> {
    let (app, _token) = app().await?;

    let (status, _) = body_to_string(
        app.oneshot(get(
            "/bridge/latest?platform=darwin-arm64",
            Some("not-a-jwt"),
        ))
        .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 401);
    Ok(())
}

// Why: a platform with no declared asset is a 404 rather than a 502. The
// distinction matters to the updater: one means "not built for you", the other
// means "upstream is broken and worth retrying".
#[tokio::test]
async fn a_platform_with_no_published_asset_is_a_not_found() -> anyhow::Result<()> {
    let (app, token) = app().await?;

    let (status, body) = body_to_string(
        app.oneshot(get("/bridge/latest?platform=solaris-sparc", Some(&token)))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 404, "{body}");
    assert!(body.contains("solaris-sparc"), "{body}");
    Ok(())
}

// Why: the digest is read from the release's cosign-signed SHA256SUMS rather
// than computed here, so the updater enforces the hash that was signed at
// publish time. Serving a real SHA256SUMS body is what makes this assert the
// whole path rather than accepting a 502 from the digest step.
#[tokio::test]
async fn the_latest_bridge_release_is_reported_with_its_version_size_and_signed_digest()
-> anyhow::Result<()> {
    let base = server().await.uri();
    mount_releases(serde_json::json!([release(
        &base,
        "bridge-v0.32.0",
        false,
        false
    )]))
    .await;
    let s = server().await;
    Mock::given(method("GET"))
        .and(path("/sums"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!("{DIGEST}  {ASSET}\n")))
        .mount(s)
        .await;

    let (app, token) = app().await?;
    let (status, body) = body_to_string(
        app.oneshot(get("/bridge/latest?platform=darwin-arm64", Some(&token)))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 200, "{body}");
    let manifest: serde_json::Value = serde_json::from_str(&body)?;
    assert_eq!(
        manifest["version"], "0.32.0",
        "the bridge- prefix is stripped so the client compares versions, not tags"
    );
    assert_eq!(manifest["size"], 1234);
    assert_eq!(
        manifest["sha256"], DIGEST,
        "the digest must come from the signed SHA256SUMS, not be recomputed"
    );
    Ok(())
}

// Why: the bridge is tagged separately from the server, so an unfiltered
// "latest release" picks up whatever shipped most recently — a server release,
// a draft, or a prerelease — and would hand an installed bridge the wrong
// binary to update itself to.
#[tokio::test]
async fn drafts_prereleases_and_foreign_tags_are_all_passed_over() -> anyhow::Result<()> {
    let base = server().await.uri();
    mount_releases(serde_json::json!([
        release(&base, "v0.41.0", false, false),
        release(&base, "bridge-v0.33.0", true, false),
        release(&base, "bridge-v0.32.5", false, true),
    ]))
    .await;

    let (app, token) = app().await?;
    let (status, body) = body_to_string(
        app.oneshot(get("/bridge/latest?platform=darwin-arm64", Some(&token)))
            .await?,
    )
    .await?;

    assert_eq!(
        status.as_u16(),
        404,
        "a server tag, a draft and a prerelease are the only candidates, so \
         none should be selected: {body}"
    );
    assert!(body.contains("bridge-v"), "{body}");
    Ok(())
}

#[tokio::test]
async fn an_upstream_error_is_a_bad_gateway_rather_than_a_not_found() -> anyhow::Result<()> {
    let s = server().await;
    s.reset().await;
    Mock::given(method("GET"))
        .and(path("/repos/systempromptio/systemprompt-core/releases"))
        .respond_with(ResponseTemplate::new(500))
        .mount(s)
        .await;

    let (app, token) = app().await?;
    let (status, body) = body_to_string(
        app.oneshot(get("/bridge/latest?platform=darwin-arm64", Some(&token)))
            .await?,
    )
    .await?;

    assert_eq!(
        status.as_u16(),
        502,
        "an upstream failure must not be reported as a missing release: {body}"
    );
    Ok(())
}

#[tokio::test]
async fn a_download_for_an_unknown_platform_is_a_not_found() -> anyhow::Result<()> {
    let (app, token) = app().await?;

    let (status, body) = body_to_string(
        app.oneshot(get("/bridge/download/solaris-sparc", Some(&token)))
            .await?,
    )
    .await?;

    assert_eq!(status.as_u16(), 404, "{body}");
    Ok(())
}

// Why: the bytes are streamed rather than buffered, and the filename the
// updater writes comes from the content-disposition this route sets — not from
// the URL it dialled. Nothing else exercises the streaming path, so a
// regression here would ship a bridge that downloads to the wrong filename or
// buffers tens of megabytes per updating client.
#[tokio::test]
async fn a_download_streams_the_asset_bytes_under_its_published_filename() -> anyhow::Result<()> {
    let s = server().await;
    s.reset().await;
    let base = s.uri();
    Mock::given(method("GET"))
        .and(path("/repos/systempromptio/systemprompt-core/releases"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!([release(&base, "bridge-v9.9.9", false, false)])),
        )
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path("/asset"))
        .respond_with(ResponseTemplate::new(200).set_body_bytes(b"tarball-bytes".to_vec()))
        .mount(s)
        .await;

    let (app, token) = app().await?;
    let response = app
        .oneshot(get("/bridge/download/darwin-arm64", Some(&token)))
        .await?;

    assert_eq!(response.status().as_u16(), 200);
    assert_eq!(
        response
            .headers()
            .get(header::CONTENT_DISPOSITION)
            .and_then(|v| v.to_str().ok()),
        Some(format!("attachment; filename=\"{ASSET}\"").as_str()),
        "the updater writes the file under the name this header carries"
    );

    let (_, body) = body_to_string(response).await?;
    assert_eq!(
        body, "tarball-bytes",
        "the proxied bytes must reach the client unaltered"
    );
    Ok(())
}

// Why: the asset fetch is a second upstream call, made after the release has
// already resolved. A failure there is the gateway's problem, not a missing
// release — reporting 404 would tell the updater to stop looking for a build
// that exists.
#[tokio::test]
async fn a_download_whose_asset_fetch_fails_upstream_is_a_bad_gateway() -> anyhow::Result<()> {
    let s = server().await;
    s.reset().await;
    let base = s.uri();
    Mock::given(method("GET"))
        .and(path("/repos/systempromptio/systemprompt-core/releases"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_json(serde_json::json!([release(&base, "bridge-v9.9.9", false, false)])),
        )
        .mount(s)
        .await;
    Mock::given(method("GET"))
        .and(path("/asset"))
        .respond_with(ResponseTemplate::new(503))
        .mount(s)
        .await;

    let (app, token) = app().await?;
    let (status, body) = body_to_string(
        app.oneshot(get("/bridge/download/darwin-arm64", Some(&token)))
            .await?,
    )
    .await?;

    assert_eq!(
        status.as_u16(),
        502,
        "an asset the gateway could not fetch is not a missing release: {body}"
    );
    Ok(())
}
