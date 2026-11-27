use semver::Version;
use systemprompt_manifest::services::{GatewayConfig, GatewayConfigSpec};
use systemprompt_models::ApiPaths;
use systemprompt_models::devices::DevicePlatform;

const INSTANCE: &str = "bridge_instance:\n  device_link_path: /bridge-auth/device-link\n  \
                        identity_path: /api/public/bridge/whoami\n";

fn parse(section: &str) -> Result<GatewayConfigSpec, serde_yaml::Error> {
    serde_yaml::from_str::<GatewayConfigSpec>(&format!("{INSTANCE}{section}"))
}

fn gateway(section: &str) -> GatewayConfig {
    parse(section).expect("gateway spec parses").resolve()
}

fn refusal(section: &str) -> String {
    parse(section)
        .expect_err("the section is refused")
        .to_string()
}

fn fingerprint() -> String {
    ["AB"; 32].join(":")
}

const BOTH_CLIENTS: &str = "  clients:\n    ios:\n      app_id: io.example.app\n      \
                            min_version: 1.2.0\n      link: https://apps.example.com/redeem\n    \
                            android:\n      app_id: io.example.android\n      min_version: \
                            1.3.0\n";

const ANDROID_PUBLISHED: &str = "bridge_releases:\n  repo: systempromptio/bridge\n  assets:\n    android: example-android.apk\n";

#[test]
fn an_instance_block_without_apps_declares_none() {
    let config = gateway("");
    config.validate().expect("validates");
    let instance = config.bridge_instance.as_ref().expect("block present");
    assert!(instance.clients.is_empty());
    assert!(instance.vouched_apps.is_empty());
    assert!(instance.clients.to_clients().is_empty());
    assert!(config.client_version_floors().is_open());
    assert!(!config.serves_app_download("android"));
}

#[test]
fn a_gateway_without_an_instance_block_vouches_for_nothing() {
    let config = GatewayConfig::default();
    assert!(config.vouched_apps().is_none());
    assert!(config.client_version_floors().is_open());
    assert!(!config.serves_app_download("android"));
}

#[test]
fn declared_clients_become_the_discovery_clients_section() {
    let config = gateway(&format!("{BOTH_CLIENTS}{ANDROID_PUBLISHED}"));
    config.validate().expect("validates");
    let clients = config
        .bridge_instance
        .as_ref()
        .expect("block present")
        .clients
        .to_clients();

    let ios = clients.ios.as_ref().expect("ios declared");
    assert_eq!(ios.app_id.as_str(), "io.example.app");
    assert_eq!(ios.min_version, Some(Version::new(1, 2, 0)));
    assert_eq!(ios.link, "https://apps.example.com/redeem");

    let android = clients.android.as_ref().expect("android declared");
    assert_eq!(android.app_id.as_str(), "io.example.android");
    assert_eq!(android.min_version, Some(Version::new(1, 3, 0)));
    assert_eq!(android.download_path, ApiPaths::bridge_download("android"));
}

#[test]
fn a_declared_android_client_with_no_published_asset_is_refused() {
    let message = gateway(BOTH_CLIENTS)
        .validate()
        .expect_err("the download the discovery document names does not exist")
        .to_string();
    assert!(
        message.contains("`bridge_releases.assets` has no `android` asset"),
        "{message}"
    );
}

#[test]
fn the_minimum_versions_are_the_per_platform_floors() {
    let floors = gateway(BOTH_CLIENTS).client_version_floors();
    assert_eq!(
        floors.floor(DevicePlatform::Ios),
        Some(&Version::new(1, 2, 0))
    );
    assert_eq!(
        floors.floor(DevicePlatform::Android),
        Some(&Version::new(1, 3, 0))
    );
    assert!(floors.admits(DevicePlatform::Ios, &Version::new(1, 2, 0)));
    assert!(!floors.admits(DevicePlatform::Android, &Version::new(1, 2, 9)));
    assert!(floors.admits(DevicePlatform::Macos, &Version::new(0, 0, 1)));
}

#[test]
fn the_phone_floors_leave_the_bridge_version_window_alone() {
    let config = serde_yaml::from_str::<GatewayConfigSpec>(&format!(
        "{INSTANCE}{BOTH_CLIENTS}bridge_releases:\n  repo: systempromptio/bridge\n  min_version: \
         0.74.0\n  max_version: 0.75.3\n  assets:\n    android: example-android.apk\n"
    ))
    .expect("gateway spec parses")
    .resolve();
    config.validate().expect("validates");
    let window = config
        .bridge_releases
        .as_ref()
        .expect("releases present")
        .version_window();
    assert_eq!(window.min, Some(Version::new(0, 74, 0)));
    assert_eq!(window.max, Some(Version::new(0, 75, 3)));
    assert!(window.admits(&Version::new(0, 75, 0)));
    assert!(!window.admits(&Version::new(1, 3, 0)));
}

#[test]
fn only_a_declared_android_app_is_offered_for_download() {
    let both = gateway(BOTH_CLIENTS);
    assert!(both.serves_app_download("android"));
    assert!(!both.serves_app_download("ios"));
    assert!(!both.serves_app_download("darwin-arm64"));
    assert!(!both.serves_app_download("Android"));

    let ios_only = gateway(
        "  clients:\n    ios:\n      app_id: io.example.app\n      link: \
         https://apps.example.com/redeem\n",
    );
    assert!(!ios_only.serves_app_download("android"));
}

#[test]
fn an_ios_link_that_is_not_https_is_refused() {
    for link in [
        "http://apps.example.com/redeem",
        "itms-services://?action=download-manifest",
    ] {
        let message = refusal(&format!(
            "  clients:\n    ios:\n      app_id: io.example.app\n      link: '{link}'\n"
        ));
        assert!(
            message.contains("it must be an https URL with a host"),
            "{link}: {message}"
        );
    }
}

#[test]
fn an_ios_link_that_is_not_a_url_is_refused() {
    let message =
        refusal("  clients:\n    ios:\n      app_id: io.example.app\n      link: /redeem\n");
    assert!(message.contains("it is not a URL"), "{message}");
}

#[test]
fn an_ios_client_without_a_link_does_not_parse() {
    assert!(parse("  clients:\n    ios:\n      app_id: io.example.app\n").is_err());
}

#[test]
fn a_minimum_version_with_build_metadata_is_refused() {
    let message = refusal(
        "  clients:\n    android:\n      app_id: io.example.app\n      min_version: \
         1.3.0+build.7\n",
    );
    assert!(
        message.contains("`clients.android.min_version` is '1.3.0+build.7'"),
        "{message}"
    );
}

#[test]
fn a_client_app_id_that_is_not_reverse_dns_is_refused() {
    assert!(parse("  clients:\n    android:\n      app_id: example\n").is_err());
}

#[test]
fn a_clients_section_rejects_unknown_keys() {
    assert!(
        parse(
            "  clients:\n    android:\n      app_id: io.example.app\n      download_path: \
             /elsewhere\n"
        )
        .is_err()
    );
    assert!(parse("  clients:\n    windows:\n      app_id: io.example.app\n").is_err());
}

#[test]
fn vouched_apps_parse_into_both_association_documents() {
    let config = gateway(&format!(
        "  vouched_apps:\n    ios:\n    - team_id: ABCDE12345\n      bundle_id: \
         io.example.app\n    android:\n    - package_name: io.example.android\n      \
         sha256_cert_fingerprints:\n      - '{}'\n",
        fingerprint()
    ));
    config.validate().expect("validates");
    let apps = config.vouched_apps().expect("declared");
    assert!(apps.apple_app_site_association().is_some());
    assert!(apps.asset_links().is_some());
}

#[test]
fn an_app_vouched_for_twice_is_refused() {
    let message = refusal(
        "  vouched_apps:\n    ios:\n    - team_id: ABCDE12345\n      bundle_id: \
         io.example.app\n    - team_id: ABCDE12345\n      bundle_id: io.example.app\n",
    );
    assert!(
        message.contains("the iOS app ABCDE12345.io.example.app is declared more than once"),
        "{message}"
    );
}

#[test]
fn an_android_app_vouched_for_without_a_fingerprint_is_refused() {
    let message = refusal(
        "  vouched_apps:\n    android:\n    - package_name: io.example.android\n      \
         sha256_cert_fingerprints: []\n",
    );
    assert!(
        message.contains("declares no certificate fingerprint"),
        "{message}"
    );
}

#[test]
fn an_ios_link_path_that_is_not_absolute_is_refused() {
    let message = refusal(
        "  vouched_apps:\n    ios:\n    - team_id: ABCDE12345\n      bundle_id: \
         io.example.app\n      link_paths: ['app/*']\n",
    );
    assert!(message.contains("it must start with '/'"), "{message}");
}

#[test]
fn a_malformed_team_id_or_fingerprint_is_refused() {
    assert!(
        parse(
            "  vouched_apps:\n    ios:\n    - team_id: abcde12345\n      bundle_id: \
             io.example.app\n"
        )
        .is_err()
    );
    assert!(
        parse(
            "  vouched_apps:\n    android:\n    - package_name: io.example.android\n      \
             sha256_cert_fingerprints: ['AB:CD']\n"
        )
        .is_err()
    );
}

#[test]
fn an_instance_with_no_apps_serialises_without_either_section() {
    let yaml = serde_yaml::to_string(&gateway("").to_spec()).expect("serialises");
    assert!(!yaml.contains("clients"), "{yaml}");
    assert!(!yaml.contains("vouched_apps"), "{yaml}");

    let declared = serde_yaml::to_string(&gateway(BOTH_CLIENTS).to_spec()).expect("serialises");
    let reparsed = serde_yaml::from_str::<GatewayConfigSpec>(&declared)
        .expect("a serialised spec parses")
        .resolve();
    assert_eq!(
        reparsed.bridge_instance,
        gateway(BOTH_CLIENTS).bridge_instance
    );
}
