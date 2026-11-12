use std::collections::BTreeMap;
use std::sync::Arc;
use std::time::Duration;

use systemprompt_api::routes::gateway::bridge_release::ReleaseFeed;
use systemprompt_manifest::Secrets;
use systemprompt_manifest::services::BridgeReleasesSpec;
use systemprompt_test_fixtures::ensure_test_secrets_bootstrap;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const DIGEST: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const ASSET: &str = "systemprompt-bridge-linux";
const PLATFORM: &str = "linux-x64";

fn secrets() -> Arc<Secrets> {
    ensure_test_secrets_bootstrap().get()
}

fn spec(server: &MockServer) -> BridgeReleasesSpec {
    let mut assets = BTreeMap::new();
    assets.insert(PLATFORM.to_owned(), ASSET.to_owned());
    BridgeReleasesSpec {
        repo: "systempromptio/bridge".to_owned(),
        token_secret: None,
        tag_prefix: "bridge-v".to_owned(),
        pinned_version: None,
        assets,
        api_base: Some(server.uri()),
        min_version: None,
        max_version: None,
        enforce_max: false,
    }
}

async fn mount(server: &MockServer, delay: Duration, release_calls: u64, sums_calls: u64) {
    let sums = format!("{}/sums", server.uri());
    Mock::given(method("GET"))
        .and(path("/repos/systempromptio/bridge/releases"))
        .respond_with(ResponseTemplate::new(200).set_delay(delay).set_body_json(
            serde_json::json!([{
                "tag_name": "bridge-v0.50.0",
                "html_url": "https://example.test/notes",
                "draft": false,
                "prerelease": false,
                "assets": [
                    { "name": ASSET, "url": "https://example.test/asset", "size": 4096 },
                    { "name": "SHA256SUMS", "url": sums, "size": 128 }
                ]
            }]),
        ))
        .expect(release_calls)
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sums"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!("{DIGEST}  {ASSET}\n")))
        .expect(sums_calls)
        .mount(server)
        .await;
}

#[tokio::test]
async fn concurrent_misses_on_one_platform_share_a_single_upstream_resolution() {
    let server = MockServer::start().await;
    mount(&server, Duration::from_millis(200), 1, 1).await;
    let feed = Arc::new(ReleaseFeed::new().expect("the feed builds"));
    let spec = Arc::new(spec(&server));

    let waiters = (0..8).map(|_| {
        let feed = Arc::clone(&feed);
        let spec = Arc::clone(&spec);
        async move { feed.resolve(&spec, &secrets(), PLATFORM).await }
    });
    let answers = futures_util::future::join_all(waiters).await;

    for answer in answers {
        let resolved = answer.expect("every waiter receives the shared resolution");
        assert_eq!(resolved.manifest.version, "0.50.0");
        assert_eq!(resolved.manifest.sha256, DIGEST);
    }
    server.verify().await;
}

#[tokio::test]
async fn a_forced_refresh_is_honoured_once_per_interval() {
    let server = MockServer::start().await;
    mount(&server, Duration::ZERO, 2, 2).await;
    let feed = ReleaseFeed::new().expect("the feed builds");
    let spec = spec(&server);

    feed.resolve(&spec, &secrets(), PLATFORM)
        .await
        .expect("the first read resolves upstream");
    for _ in 0..4 {
        let refreshed = feed
            .resolve_fresh(&spec, &secrets(), PLATFORM)
            .await
            .expect("a refresh always answers");
        assert_eq!(refreshed.manifest.version, "0.50.0");
    }
    server.verify().await;
}

#[tokio::test]
async fn an_unconfigured_platform_is_refused_before_any_cache_or_upstream_work() {
    let server = MockServer::start().await;
    mount(&server, Duration::ZERO, 0, 0).await;
    let feed = ReleaseFeed::new().expect("the feed builds");

    let error = feed
        .resolve_fresh(&spec(&server), &secrets(), "plan9-mips")
        .await
        .expect_err("a platform the spec does not publish is refused");

    assert!(error.to_string().contains("plan9-mips"), "{error}");
    server.verify().await;
}
