use serde_json::json;
use systemprompt_models::association::{
    AppleAppId, AppleTeamId, Sha256CertFingerprint, VouchedApps, VouchedAppsError,
};

const FINGERPRINT: &str = "14:6D:E9:83:C5:73:06:50:D8:EE:B9:95:2F:34:FC:64:16:A0:83:42:E6:1D:BE:\
                           A8:8A:04:96:B2:3F:CF:44:E5";

fn other_fingerprint() -> String {
    FINGERPRINT.replacen("14", "15", 1)
}

fn declared() -> VouchedApps {
    serde_yaml::from_str(&format!(
        "ios:\n  - team_id: ABCDE12345\n    bundle_id: io.example.app\n    link_paths: \
         ['/runs/*', '/approvals/*']\n  - team_id: ABCDE12345\n    bundle_id: \
         io.example.beta\nandroid:\n  - package_name: io.example.app\n    \
         sha256_cert_fingerprints:\n      - '{FINGERPRINT}'\n"
    ))
    .expect("declaration parses")
}

#[test]
fn an_instance_with_no_apps_has_neither_document() {
    let none = VouchedApps::default();
    assert!(none.is_empty());
    assert!(none.validate().is_ok());
    assert!(none.apple_app_site_association().is_none());
    assert!(none.asset_links().is_none());
    assert_eq!(serde_json::to_value(&none).unwrap(), json!({}));
}

#[test]
fn each_document_needs_an_app_of_its_own_vendor() {
    let mut apps = declared();
    apps.android.clear();
    assert!(apps.apple_app_site_association().is_some());
    assert!(apps.asset_links().is_none());

    let mut apps = declared();
    apps.ios.clear();
    assert!(apps.apple_app_site_association().is_none());
    assert!(apps.asset_links().is_some());
}

#[test]
fn the_apple_document_is_the_vendor_format() {
    let apps = declared();
    apps.validate().expect("valid");
    let document = apps.apple_app_site_association().expect("ios declared");
    assert_eq!(
        serde_json::to_value(&document).unwrap(),
        json!({
            "applinks": {
                "details": [{
                    "appIDs": ["ABCDE12345.io.example.app"],
                    "components": [{"/": "/runs/*"}, {"/": "/approvals/*"}],
                }],
            },
            "webcredentials": {
                "apps": ["ABCDE12345.io.example.app", "ABCDE12345.io.example.beta"],
            },
        })
    );
}

#[test]
fn an_apple_app_with_no_link_paths_shares_credentials_only() {
    let mut apps = declared();
    apps.ios.remove(0);
    let value = serde_json::to_value(apps.apple_app_site_association().unwrap()).unwrap();
    assert_eq!(
        value,
        json!({"webcredentials": {"apps": ["ABCDE12345.io.example.beta"]}})
    );
}

#[test]
fn the_android_document_is_the_vendor_format() {
    let document = declared().asset_links().expect("android declared");
    assert_eq!(
        serde_json::to_value(&document).unwrap(),
        json!([{
            "relation": [
                "delegate_permission/common.handle_all_urls",
                "delegate_permission/common.get_login_creds",
            ],
            "target": {
                "namespace": "android_app",
                "package_name": "io.example.app",
                "sha256_cert_fingerprints": [FINGERPRINT],
            },
        }])
    );
}

#[test]
fn both_documents_read_back_what_they_wrote() {
    let apps = declared();
    let apple = apps.apple_app_site_association().unwrap();
    let json = serde_json::to_string(&apple).unwrap();
    assert_eq!(serde_json::from_str::<serde_json::Value>(&json).unwrap(), {
        serde_json::to_value(
            serde_json::from_str::<systemprompt_models::association::AppleAppSiteAssociation>(
                &json,
            )
            .unwrap(),
        )
        .unwrap()
    });
    let android = apps.asset_links().unwrap();
    let json = serde_json::to_string(&android).unwrap();
    assert_eq!(
        serde_json::from_str::<systemprompt_models::association::AssetLinks>(&json).unwrap(),
        android
    );
}

#[test]
fn a_fingerprint_is_thirty_two_hex_pairs_normalised_to_uppercase() {
    let lower = FINGERPRINT.to_ascii_lowercase();
    assert_eq!(
        Sha256CertFingerprint::try_new(&lower).unwrap().as_str(),
        FINGERPRINT
    );
    for invalid in [
        "",
        "14:6D",
        &FINGERPRINT.replace(':', ""),
        &FINGERPRINT.replacen("14", "1G", 1),
        &format!("{FINGERPRINT}:00"),
        &FINGERPRINT.replacen("14:", "146:", 1),
    ] {
        assert!(
            Sha256CertFingerprint::try_new(invalid).is_err(),
            "{invalid}"
        );
    }
}

#[test]
fn a_team_id_is_ten_uppercase_alphanumerics() {
    assert!(AppleTeamId::try_new("ABCDE12345").is_ok());
    for invalid in ["", "ABCDE1234", "ABCDE123456", "abcde12345", "ABCDE-2345"] {
        assert!(AppleTeamId::try_new(invalid).is_err(), "{invalid}");
    }
}

#[test]
fn an_app_id_is_the_team_id_then_the_bundle_id() {
    let app_id: AppleAppId = "ABCDE12345.io.example.app".parse().unwrap();
    assert_eq!(app_id.team_id().as_str(), "ABCDE12345");
    assert_eq!(app_id.bundle_id().as_str(), "io.example.app");
    assert_eq!(app_id.to_string(), "ABCDE12345.io.example.app");
    assert!("ABCDE12345".parse::<AppleAppId>().is_err());
    assert!("abc.io.example.app".parse::<AppleAppId>().is_err());
    assert!("ABCDE12345.app".parse::<AppleAppId>().is_err());
}

#[test]
fn a_declaration_refuses_malformed_identifiers_and_unknown_keys() {
    for yaml in [
        "ios:\n  - team_id: short\n    bundle_id: io.example.app\n",
        "ios:\n  - team_id: ABCDE12345\n    bundle_id: app\n",
        "android:\n  - package_name: io.example.app\n    sha256_cert_fingerprints: ['AA:BB']\n",
        "android:\n  - package_name: io.example.app\n",
        "ios:\n  - team_id: ABCDE12345\n    bundle_id: io.example.app\n    colour: red\n",
        "web: []\n",
    ] {
        assert!(serde_yaml::from_str::<VouchedApps>(yaml).is_err(), "{yaml}");
    }
}

#[test]
fn an_android_app_needs_a_fingerprint() {
    let mut apps = declared();
    apps.android[0].sha256_cert_fingerprints.clear();
    assert!(matches!(
        apps.validate(),
        Err(VouchedAppsError::NoFingerprints { package_name })
            if package_name.as_str() == "io.example.app"
    ));
}

#[test]
fn duplicates_are_refused() {
    let mut apps = declared();
    let repeated = apps.android[0].sha256_cert_fingerprints[0].clone();
    apps.android[0].sha256_cert_fingerprints.push(repeated);
    assert!(matches!(
        apps.validate(),
        Err(VouchedAppsError::DuplicateFingerprint { .. })
    ));

    let mut apps = declared();
    let mut second = apps.android[0].clone();
    second.sha256_cert_fingerprints =
        vec![Sha256CertFingerprint::try_new(other_fingerprint()).unwrap()];
    apps.android.push(second);
    assert!(matches!(
        apps.validate(),
        Err(VouchedAppsError::DuplicateAndroidApp { .. })
    ));

    let mut apps = declared();
    let again = apps.ios[1].clone();
    apps.ios.push(again);
    assert!(matches!(
        apps.validate(),
        Err(VouchedAppsError::DuplicateAppleApp { .. })
    ));
}

#[test]
fn a_link_path_is_absolute() {
    let mut apps = declared();
    apps.ios[0].link_paths.push("runs/*".to_owned());
    assert!(matches!(
        apps.validate(),
        Err(VouchedAppsError::LinkPathNotAbsolute { path, .. }) if path == "runs/*"
    ));
}
