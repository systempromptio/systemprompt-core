//! `android` as a platform of the release feed: the signed APK is one more
//! asset of the install's release, resolved with the digest its SHA256SUMS
//! publishes and served as an Android package.

use std::collections::BTreeMap;

use axum::http::StatusCode;
use systemprompt_gateway_entry::http::bridge::release::{
    ANDROID_PACKAGE_CONTENT_TYPE, ReleaseError, ReleaseFeed, asset_content_type,
};
use systemprompt_manifest::services::BridgeReleasesSpec;
use systemprompt_test_fixtures::ensure_test_secrets_bootstrap;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

const BRIDGE_DIGEST: &str = "e3b0c44298fc1c149afbf4c8996fb92427ae41e4649b934ca495991b7852b855";
const APK_DIGEST: &str = "9f86d081884c7d659a2feaa0c55ad015a3bf4f1b2b0b822cd15d6c15b0f00a08";
const BRIDGE_ASSET: &str = "systemprompt-bridge-linux";
const APK_ASSET: &str = "example-android.apk";

fn secrets() -> systemprompt_gateway::surface::views::BridgeReleaseSource {
    systemprompt_gateway::surface::views::BridgeReleaseSource::new(ensure_test_secrets_bootstrap())
}

fn spec(server: &MockServer, platforms: &[(&str, &str)]) -> BridgeReleasesSpec {
    BridgeReleasesSpec {
        repo: "systempromptio/bridge".to_owned(),
        token_secret: None,
        tag_prefix: "bridge-v".to_owned(),
        pinned_version: None,
        assets: platforms
            .iter()
            .map(|(platform, asset)| ((*platform).to_owned(), (*asset).to_owned()))
            .collect::<BTreeMap<_, _>>(),
        api_base: Some(server.uri()),
        min_version: None,
        max_version: None,
        enforce_max: false,
    }
}

async fn mount_release(server: &MockServer) {
    let body = serde_json::json!([{
        "tag_name": "bridge-v0.50.0",
        "html_url": "https://example.test/notes",
        "draft": false,
        "prerelease": false,
        "assets": [
            { "name": BRIDGE_ASSET, "url": "https://example.test/bridge", "size": 4096 },
            { "name": APK_ASSET, "url": "https://example.test/apk", "size": 8192 },
            { "name": "SHA256SUMS", "url": format!("{}/sums", server.uri()), "size": 128 }
        ]
    }]);
    Mock::given(method("GET"))
        .and(path("/repos/systempromptio/bridge/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sums"))
        .respond_with(ResponseTemplate::new(200).set_body_string(format!(
            "{BRIDGE_DIGEST}  {BRIDGE_ASSET}\n{APK_DIGEST}  {APK_ASSET}\n"
        )))
        .mount(server)
        .await;
}

#[tokio::test]
async fn the_android_platform_resolves_the_apk_with_its_published_checksum() {
    let server = MockServer::start().await;
    mount_release(&server).await;
    let spec = spec(
        &server,
        &[("linux-x64", BRIDGE_ASSET), ("android", APK_ASSET)],
    );

    let feed = ReleaseFeed::new().expect("the feed builds");
    let android = feed
        .resolve(&spec, &secrets(), "android")
        .await
        .expect("the android platform resolves");

    assert_eq!(android.manifest.version, "0.50.0");
    assert_eq!(android.manifest.sha256, APK_DIGEST);
    assert_eq!(android.manifest.size, 8192);
    assert_eq!(android.asset.name, APK_ASSET);
    assert_eq!(android.asset.url, "https://example.test/apk");

    let desktop = feed
        .resolve(&spec, &secrets(), "linux-x64")
        .await
        .expect("the desktop platform still resolves");
    assert_eq!(desktop.manifest.sha256, BRIDGE_DIGEST);
    assert_eq!(desktop.asset.name, BRIDGE_ASSET);
}

#[tokio::test]
async fn an_install_that_publishes_no_apk_has_no_android_platform() {
    let server = MockServer::start().await;
    let spec = spec(&server, &[("linux-x64", BRIDGE_ASSET)]);

    let error = ReleaseFeed::new()
        .expect("the feed builds")
        .resolve(&spec, &secrets(), "android")
        .await
        .expect_err("no asset is declared for android");

    assert!(
        matches!(&error, ReleaseError::UnknownPlatform { platform } if platform == "android"),
        "{error:?}"
    );
    assert_eq!(error.status(), StatusCode::NOT_FOUND);
    assert_eq!(error.error_key(), "platform_not_published");
}

#[tokio::test]
async fn an_apk_missing_from_the_checksum_file_is_refused() {
    let server = MockServer::start().await;
    let body = serde_json::json!([{
        "tag_name": "bridge-v0.50.0",
        "draft": false,
        "prerelease": false,
        "assets": [
            { "name": APK_ASSET, "url": "https://example.test/apk", "size": 8192 },
            { "name": "SHA256SUMS", "url": format!("{}/sums", server.uri()), "size": 64 }
        ]
    }]);
    Mock::given(method("GET"))
        .and(path("/repos/systempromptio/bridge/releases"))
        .respond_with(ResponseTemplate::new(200).set_body_json(body))
        .mount(&server)
        .await;
    Mock::given(method("GET"))
        .and(path("/sums"))
        .respond_with(
            ResponseTemplate::new(200)
                .set_body_string(format!("{BRIDGE_DIGEST}  {BRIDGE_ASSET}\n")),
        )
        .mount(&server)
        .await;

    let error = ReleaseFeed::new()
        .expect("the feed builds")
        .resolve(
            &spec(&server, &[("android", APK_ASSET)]),
            &secrets(),
            "android",
        )
        .await
        .expect_err("an APK with no published checksum cannot be verified");

    assert!(
        matches!(&error, ReleaseError::ChecksumMissing { asset } if asset == APK_ASSET),
        "{error:?}"
    );
    assert_eq!(error.status(), StatusCode::BAD_GATEWAY);
}

#[test]
fn only_the_android_platform_is_served_as_an_android_package() {
    assert_eq!(
        ANDROID_PACKAGE_CONTENT_TYPE,
        "application/vnd.android.package-archive"
    );
    assert_eq!(asset_content_type("android"), ANDROID_PACKAGE_CONTENT_TYPE);
    for platform in ["darwin-arm64", "linux-x64", "windows-x64", "ios", "Android"] {
        assert_eq!(
            asset_content_type(platform),
            "application/octet-stream",
            "{platform}"
        );
    }
}
