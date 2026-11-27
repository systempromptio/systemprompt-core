//! `Instances::discover` against a stub instance.

use serde_json::json;
use systemprompt_mobile::{AppRelease, Instances, MobileError};
use systemprompt_models::ApiPaths;
use wiremock::matchers::{method, path};
use wiremock::{Mock, MockServer, ResponseTemplate};

async fn instance(response: ResponseTemplate) -> MockServer {
    let server = MockServer::start().await;
    Mock::given(method("GET"))
        .and(path(ApiPaths::WELLKNOWN_BRIDGE))
        .respond_with(response)
        .mount(&server)
        .await;
    server
}

fn document(contract: &str) -> serde_json::Value {
    json!({
        "contract": contract,
        "device_link_path": "/api/v1/bridge/device-link",
        "identity_path": "/api/v1/bridge/identity",
        "branding": { "display_name": "Enterprise Demo", "sign_in_hint": "Use your work account" },
        "enabled_hosts": [],
        "clients": {
            "ios": { "app_id": "io.systemprompt.mobile", "link": "https://testflight.example/join/abc" },
            "android": {
                "app_id": "io.systemprompt.mobile",
                "min_version": "0.70.0",
                "download_path": "/v1/bridge/download/android"
            }
        }
    })
}

#[tokio::test]
async fn discovery_maps_the_document_and_resolves_the_android_download() {
    let server = instance(ResponseTemplate::new(200).set_body_json(document("1"))).await;
    let instances = Instances::new();

    let found = instances.discover(server.uri()).await.expect("a discovery");

    assert_eq!(found.address, server.uri());
    assert_eq!(found.display_name.as_deref(), Some("Enterprise Demo"));
    assert_eq!(found.sign_in_hint.as_deref(), Some("Use your work account"));
    assert_eq!(
        found.ios,
        Some(AppRelease {
            app_id: "io.systemprompt.mobile".to_owned(),
            min_version: None,
            location: "https://testflight.example/join/abc".to_owned(),
        })
    );
    assert_eq!(
        found.android,
        Some(AppRelease {
            app_id: "io.systemprompt.mobile".to_owned(),
            min_version: Some("0.70.0".to_owned()),
            location: format!("{}/v1/bridge/download/android", server.uri()),
        })
    );
    assert_eq!(instances.active(), Some(found));
}

#[tokio::test]
async fn an_instance_with_no_apps_configured_lists_none() {
    let mut body = document("1");
    body.as_object_mut().expect("an object").remove("clients");
    let server = instance(ResponseTemplate::new(200).set_body_json(body)).await;

    let found = Instances::new().discover(server.uri()).await.expect("a discovery");

    assert_eq!(found.ios, None);
    assert_eq!(found.android, None);
}

#[tokio::test]
async fn an_instance_without_discovery_is_not_found_and_does_not_become_active() {
    let server = instance(ResponseTemplate::new(404)).await;
    let instances = Instances::new();

    assert_eq!(instances.discover(server.uri()).await, Err(MobileError::NotFound));
    assert_eq!(instances.active(), None);
}

#[tokio::test]
async fn a_server_failure_is_a_refusal_with_its_status() {
    let server = instance(ResponseTemplate::new(503)).await;

    assert_eq!(
        Instances::new().discover(server.uri()).await,
        Err(MobileError::Refused { status: 503 })
    );
}

#[tokio::test]
async fn a_contract_this_build_does_not_read_is_too_old() {
    let server = instance(ResponseTemplate::new(200).set_body_json(document("2"))).await;

    assert_eq!(
        Instances::new().discover(server.uri()).await,
        Err(MobileError::VersionTooOld)
    );
}

#[tokio::test]
async fn an_unreachable_instance_is_a_network_failure() {
    assert_eq!(
        Instances::new().discover("http://127.0.0.1:1".to_owned()).await,
        Err(MobileError::Network)
    );
}

#[tokio::test]
async fn an_address_that_is_not_an_origin_never_reaches_the_network() {
    assert_eq!(
        Instances::new().discover("https://demo.systemprompt.io/admin".to_owned()).await,
        Err(MobileError::InvalidAddress)
    );
}
