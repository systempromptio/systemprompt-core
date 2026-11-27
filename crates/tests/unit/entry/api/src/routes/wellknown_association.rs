//! The two app association documents as the well-known routes build them:
//! exactly the vendor's JSON for the apps an instance vouches for, and no
//! document at all for a vendor it declares no app of.

use serde_json::json;
use systemprompt_api::routes::wellknown::ASSOCIATION_CACHE_CONTROL;
use systemprompt_api::routes::wellknown::android::asset_links;
use systemprompt_api::routes::wellknown::apple::apple_app_site_association;
use systemprompt_manifest::services::gateway::GatewayState;
use systemprompt_manifest::services::{GatewayConfig, GatewayConfigSpec, ServicesConfig};

fn fingerprint() -> String {
    ["AB"; 32].join(":")
}

fn vouching(vouched_apps: serde_json::Value) -> ServicesConfig {
    let spec: GatewayConfigSpec = serde_json::from_value(json!({
        "bridge_instance": {
            "device_link_path": "/bridge-auth/device-link",
            "identity_path": "/api/public/bridge/whoami",
            "vouched_apps": vouched_apps,
        }
    }))
    .expect("gateway spec parses");
    ServicesConfig {
        gateway: Some(GatewayState::Resolved(spec.resolve())),
        ..ServicesConfig::default()
    }
}

fn both_vendors() -> ServicesConfig {
    vouching(json!({
        "ios": [
            {
                "team_id": "ABCDE12345",
                "bundle_id": "io.example.app",
                "link_paths": ["/app/*", "/invite/*"],
            },
            { "team_id": "ABCDE12345", "bundle_id": "io.example.beta" },
        ],
        "android": [
            {
                "package_name": "io.example.android",
                "sha256_cert_fingerprints": [fingerprint(), fingerprint().to_lowercase().replace("ab", "cd")],
            },
        ],
    }))
}

#[test]
fn an_instance_without_a_gateway_section_serves_neither_document() {
    let services = ServicesConfig::default();
    assert!(apple_app_site_association(&services).is_none());
    assert!(asset_links(&services).is_none());
}

#[test]
fn an_instance_without_an_instance_block_serves_neither_document() {
    let services = ServicesConfig {
        gateway: Some(GatewayState::Resolved(GatewayConfig::default())),
        ..ServicesConfig::default()
    };
    assert!(apple_app_site_association(&services).is_none());
    assert!(asset_links(&services).is_none());
}

#[test]
fn an_instance_that_vouches_for_no_app_serves_neither_document() {
    let services = vouching(json!({}));
    assert!(apple_app_site_association(&services).is_none());
    assert!(asset_links(&services).is_none());
}

#[test]
fn the_apple_document_is_exactly_the_vendor_shape() {
    let document = apple_app_site_association(&both_vendors()).expect("an iOS app is declared");
    assert_eq!(
        serde_json::to_value(&document).expect("serialise"),
        json!({
            "applinks": {
                "details": [
                    {
                        "appIDs": ["ABCDE12345.io.example.app"],
                        "components": [{ "/": "/app/*" }, { "/": "/invite/*" }],
                    },
                ],
            },
            "webcredentials": {
                "apps": ["ABCDE12345.io.example.app", "ABCDE12345.io.example.beta"],
            },
        })
    );
}

#[test]
fn an_apple_app_that_opens_no_link_shares_credentials_only() {
    let services = vouching(json!({
        "ios": [{ "team_id": "ABCDE12345", "bundle_id": "io.example.app" }],
    }));
    assert_eq!(
        serde_json::to_value(apple_app_site_association(&services).expect("declared"))
            .expect("serialise"),
        json!({ "webcredentials": { "apps": ["ABCDE12345.io.example.app"] } })
    );
    assert!(asset_links(&services).is_none());
}

#[test]
fn the_android_document_is_exactly_the_vendor_shape() {
    let document = asset_links(&both_vendors()).expect("an Android app is declared");
    assert_eq!(
        serde_json::to_value(&document).expect("serialise"),
        json!([
            {
                "relation": [
                    "delegate_permission/common.handle_all_urls",
                    "delegate_permission/common.get_login_creds",
                ],
                "target": {
                    "namespace": "android_app",
                    "package_name": "io.example.android",
                    "sha256_cert_fingerprints": [fingerprint(), fingerprint().replace("AB", "CD")],
                },
            },
        ])
    );
}

#[test]
fn an_instance_with_only_an_android_app_serves_no_apple_document() {
    let services = vouching(json!({
        "android": [{
            "package_name": "io.example.android",
            "sha256_cert_fingerprints": [fingerprint()],
        }],
    }));
    assert!(apple_app_site_association(&services).is_none());
    assert!(asset_links(&services).is_some());
}

#[test]
fn the_documents_are_publicly_cacheable() {
    assert_eq!(ASSOCIATION_CACHE_CONTROL, "public, max-age=3600");
}
