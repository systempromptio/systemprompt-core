use std::collections::BTreeMap;
use std::time::Duration;

use axum::http::StatusCode;
use systemprompt_api::routes::gateway::bridge_release::ReleaseFeed;
use systemprompt_models::services::BridgeReleasesSpec;
use wiremock::matchers::{method, path, query_param};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DIGEST: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const ASSET: &str = "systemprompt-bridge-linux";

fn spec(server: &MockServer) -> BridgeReleasesSpec {
    let mut assets = BTreeMap::new();
    assets.insert("linux-x64".to_owned(), ASSET.to_owned());
    BridgeReleasesSpec {
        repo: "systempromptio/bridge".to_owned(),
        token_env: None,
        tag_prefix: "bridge-v".to_owned(),
        pinned_version: None,
        assets,
        api_base: Some(server.uri()),
    }
}

fn release(tag: &str, sums_url: &str, asset_url: &str) -> serde_json::Value {
    serde_json::json!({
        "tag_name": tag,
        "html_url": "https://example.test/notes",
        "draft": false,
        "prerelease": false,
        "assets": [
            { "name": ASSET, "url": asset_url, "size": 4096 },
            { "name": "SHA256SUMS", "url": sums_url, "size": 128 }
        ]
    })
}

async fn mount_releases(server: &MockServer, body: serde_json::Value) {
    Mock::given(method("GET"))
        .and(path("/repos/systempromptio/bridge/releases"))
        .and(query_param("per_page", "30"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
}

async fn mount_sums(server: &MockServer, body: &str) {
    Mock::given(method("GET"))
        .and(path("/sums"))
        .respond_with(ResponseTemplate::new(200).set_body_string(body.to_owned()))
        .mount(server)
        .await;
}

#[tokio::test]
async fn a_resolved_release_strips_the_tag_prefix_and_carries_the_published_digest() {
    let server = MockServer::start().await;
    let sums = format!("{}/sums", server.uri());
    mount_releases(
        &server,
        serde_json::json!([release(
            "bridge-v0.50.0",
            &sums,
            "https://example.test/asset"
        )]),
    )
    .await;
    mount_sums(&server, &format!("{DIGEST}  {ASSET}\n")).await;

    let resolved = ReleaseFeed::default()
        .resolve(&spec(&server), "linux-x64")
        .await
        .expect("the feed resolves a published release");

    assert_eq!(resolved.manifest.version, "0.50.0");
    assert_eq!(resolved.manifest.sha256, DIGEST);
    assert_eq!(resolved.manifest.size, 4096);
    assert_eq!(
        resolved.manifest.notes_url.as_deref(),
        Some("https://example.test/notes")
    );
    assert_eq!(resolved.asset.name, ASSET);
    assert_eq!(resolved.asset.url, "https://example.test/asset");
}

#[tokio::test]
async fn a_second_resolution_within_the_ttl_does_not_call_github_again() {
    let server = MockServer::start().await;
    let sums = format!("{}/sums", server.uri());
    Mock::given(method("GET"))
        .and(path("/repos/systempromptio/bridge/releases"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([release(
                "bridge-v0.50.0",
                &sums,
                "https://example.test/asset"
            )])),
        )
        .expect(1)
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sums"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!("{DIGEST}  {ASSET}\n")))
        .expect(1)
        .mount(&server)
        .await;

    let feed = ReleaseFeed::default();
    let spec = spec(&server);
    let first = feed.resolve(&spec, "linux-x64").await.expect("first");
    let second = feed.resolve(&spec, "linux-x64").await.expect("cached");

    assert_eq!(first.manifest.version, second.manifest.version);
    assert_eq!(first.manifest.sha256, second.manifest.sha256);
}

#[tokio::test]
async fn a_github_outage_after_a_good_answer_serves_the_last_known_release() {
    let server = MockServer::start().await;
    let sums = format!("{}/sums", server.uri());
    Mock::given(method("GET"))
        .and(path("/repos/systempromptio/bridge/releases"))
        .respond_with(
            ResponseTemplate::new(200).set_body_json(serde_json::json!([release(
                "bridge-v0.50.0",
                &sums,
                "https://example.test/asset"
            )])),
        )
        .up_to_n_times(1)
        .mount(&server)
        .await;
    mount_sums(&server, &format!("{DIGEST}  {ASSET}\n")).await;

    let feed = ReleaseFeed::with_ttl(Duration::from_millis(0));
    let spec = spec(&server);
    feed.resolve(&spec, "linux-x64").await.expect("first");

    Mock::given(method("GET"))
        .and(path("/repos/systempromptio/bridge/releases"))
        .respond_with(ResponseTemplate::new(503))
        .mount(&server)
        .await;

    let stale = feed
        .resolve(&spec, "linux-x64")
        .await
        .expect("a github blip must not fail the update check");

    assert_eq!(stale.manifest.version, "0.50.0");
    assert_eq!(stale.manifest.sha256, DIGEST);
}

#[tokio::test]
async fn a_sums_fetch_failure_after_a_good_answer_serves_the_last_known_digest() {
    let server = MockServer::start().await;
    let sums = format!("{}/sums", server.uri());
    mount_releases(
        &server,
        serde_json::json!([release(
            "bridge-v0.50.0",
            &sums,
            "https://example.test/asset"
        )]),
    )
    .await;
    Mock::given(method("GET"))
        .and(path("/sums"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!("{DIGEST}  {ASSET}\n")))
        .up_to_n_times(1)
        .mount(&server)
        .await;

    let feed = ReleaseFeed::with_ttl(Duration::from_millis(0));
    let spec = spec(&server);
    feed.resolve(&spec, "linux-x64").await.expect("first");

    Mock::given(method("GET"))
        .and(path("/sums"))
        .respond_with(ResponseTemplate::new(200).set_body_string("no entry for this asset\n"))
        .mount(&server)
        .await;

    let stale = feed
        .resolve(&spec, "linux-x64")
        .await
        .expect("stale digest");

    assert_eq!(stale.manifest.sha256, DIGEST);
}

#[tokio::test]
async fn a_release_without_sha256sums_is_refused_when_nothing_is_cached() {
    let server = MockServer::start().await;
    mount_releases(
        &server,
        serde_json::json!([{
            "tag_name": "bridge-v0.50.0",
            "draft": false,
            "prerelease": false,
            "assets": [{ "name": ASSET, "url": "https://example.test/asset", "size": 10 }]
        }]),
    )
    .await;

    let (status, message) = ReleaseFeed::default()
        .resolve(&spec(&server), "linux-x64")
        .await
        .expect_err("a release with no checksum file cannot be trusted");

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(message.contains("publishes no SHA256SUMS"), "{message}");
}

#[tokio::test]
async fn an_unpublished_platform_is_a_not_found_before_github_is_called() {
    let server = MockServer::start().await;

    let (status, message) = ReleaseFeed::default()
        .resolve_asset(&spec(&server), "solaris-sparc")
        .await
        .expect_err("an unknown platform has no build");

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(message.contains("solaris-sparc"), "{message}");
}

#[tokio::test]
async fn a_release_missing_the_platform_asset_names_the_asset_it_wanted() {
    let server = MockServer::start().await;
    mount_releases(
        &server,
        serde_json::json!([{
            "tag_name": "bridge-v0.50.0",
            "draft": false,
            "prerelease": false,
            "assets": [{ "name": "SHA256SUMS", "url": "https://example.test/sums", "size": 1 }]
        }]),
    )
    .await;

    let (status, message) = ReleaseFeed::default()
        .resolve_asset(&spec(&server), "linux-x64")
        .await
        .expect_err("a release without the platform asset cannot be served");

    assert_eq!(status, StatusCode::NOT_FOUND);
    assert!(message.contains(ASSET), "{message}");
}

#[tokio::test]
async fn drafts_and_prereleases_are_skipped_in_favour_of_the_newest_published_release() {
    let server = MockServer::start().await;
    let sums = format!("{}/sums", server.uri());
    mount_releases(
        &server,
        serde_json::json!([
            {
                "tag_name": "bridge-v0.51.0",
                "draft": true,
                "prerelease": false,
                "assets": [{ "name": ASSET, "url": "https://example.test/draft", "size": 1 }]
            },
            {
                "tag_name": "bridge-v0.50.1",
                "draft": false,
                "prerelease": true,
                "assets": [{ "name": ASSET, "url": "https://example.test/pre", "size": 1 }]
            },
            release("bridge-v0.50.0", &sums, "https://example.test/asset")
        ]),
    )
    .await;
    mount_sums(&server, &format!("{DIGEST}  {ASSET}\n")).await;

    let resolved = ReleaseFeed::default()
        .resolve(&spec(&server), "linux-x64")
        .await
        .expect("the newest published release resolves");

    assert_eq!(resolved.manifest.version, "0.50.0");
    assert_eq!(resolved.asset.url, "https://example.test/asset");
}

#[tokio::test]
async fn a_pinned_version_is_fetched_by_tag_rather_than_from_the_release_list() {
    let server = MockServer::start().await;
    let sums = format!("{}/sums", server.uri());
    Mock::given(method("GET"))
        .and(path(
            "/repos/systempromptio/bridge/releases/tags/bridge-v0.49.0",
        ))
        .respond_with(ResponseTemplate::new(200).set_body_json(release(
            "bridge-v0.49.0",
            &sums,
            "https://example.test/pinned",
        )))
        .expect(1)
        .mount(&server)
        .await;
    mount_sums(&server, &format!("{DIGEST}  {ASSET}\n")).await;

    let mut spec = spec(&server);
    spec.pinned_version = Some("0.49.0".to_owned());

    let resolved = ReleaseFeed::default()
        .resolve(&spec, "linux-x64")
        .await
        .expect("a pinned release resolves by tag");

    assert_eq!(resolved.manifest.version, "0.49.0");
    assert_eq!(resolved.asset.url, "https://example.test/pinned");
}

#[tokio::test]
async fn a_github_error_with_nothing_cached_is_reported_as_a_bad_gateway() {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path("/repos/systempromptio/bridge/releases"))
        .respond_with(ResponseTemplate::new(500))
        .mount(&server)
        .await;

    let (status, message) = ReleaseFeed::default()
        .resolve_asset(&spec(&server), "linux-x64")
        .await
        .expect_err("no cached answer means the failure surfaces");

    assert_eq!(status, StatusCode::BAD_GATEWAY);
    assert!(message.contains("github returned 500"), "{message}");
}
