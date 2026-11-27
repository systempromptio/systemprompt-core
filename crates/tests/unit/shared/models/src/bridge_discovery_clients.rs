use semver::Version;
use serde_json::json;
use systemprompt_models::ApiPaths;
use systemprompt_models::bridge::clients::{
    AndroidClient, ClientVersionFloors, DiscoveryClients, IosClient,
};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::instance::{
    BridgeDiscovery, DISCOVERY_CONTRACT, InstanceBranding, WindowStatus,
};
use systemprompt_models::devices::{AppIdentifier, DevicePlatform};

fn app_id() -> AppIdentifier {
    AppIdentifier::try_new("io.example.app").unwrap()
}

fn discovery(clients: DiscoveryClients) -> BridgeDiscovery {
    BridgeDiscovery {
        contract: DISCOVERY_CONTRACT.to_owned(),
        device_link_path: "/bridge-auth/device-link".to_owned(),
        identity_path: "/v1/bridge/whoami".to_owned(),
        branding: InstanceBranding::default(),
        enabled_hosts: vec![HostKind::ClaudeCode],
        clients,
    }
}

fn both() -> DiscoveryClients {
    DiscoveryClients {
        ios: Some(IosClient {
            app_id: app_id(),
            min_version: Some(Version::new(1, 2, 0)),
            link: "https://apps.example.com/redeem".to_owned(),
        }),
        android: Some(AndroidClient::served_by_instance(
            app_id(),
            Some(Version::new(1, 3, 0)),
        )),
    }
}

#[test]
fn a_document_with_no_clients_keeps_its_serialised_shape() {
    let value = serde_json::to_value(discovery(DiscoveryClients::default())).unwrap();
    assert_eq!(
        value,
        json!({
            "contract": "1",
            "device_link_path": "/bridge-auth/device-link",
            "identity_path": "/v1/bridge/whoami",
            "branding": {},
            "enabled_hosts": ["claude-code"],
        })
    );
}

#[test]
fn a_document_written_before_the_clients_section_still_parses() {
    let document: BridgeDiscovery = serde_json::from_value(json!({
        "contract": "1",
        "device_link_path": "/a",
        "identity_path": "/b",
    }))
    .unwrap();
    assert!(document.clients.is_empty());
    assert!(document.clients.version_floors().is_open());
}

#[test]
fn the_clients_section_lists_each_configured_platform() {
    let document = discovery(both());
    let value = serde_json::to_value(&document).unwrap();
    assert_eq!(
        value["clients"],
        json!({
            "ios": {
                "app_id": "io.example.app",
                "min_version": "1.2.0",
                "link": "https://apps.example.com/redeem",
            },
            "android": {
                "app_id": "io.example.app",
                "min_version": "1.3.0",
                "download_path": "/v1/bridge/download/android",
            },
        })
    );
    assert_eq!(
        serde_json::from_value::<BridgeDiscovery>(value).unwrap(),
        document
    );
}

#[test]
fn one_platform_alone_is_listed_alone() {
    let mut clients = both();
    clients.ios = None;
    clients.android.as_mut().unwrap().min_version = None;
    assert!(!clients.is_empty());
    assert_eq!(
        serde_json::to_value(&clients).unwrap(),
        json!({
            "android": {
                "app_id": "io.example.app",
                "download_path": "/v1/bridge/download/android",
            },
        })
    );
}

#[test]
fn the_android_download_is_the_instance_release_route() {
    let client = AndroidClient::served_by_instance(app_id(), None);
    assert_eq!(client.download_path, ApiPaths::bridge_download("android"));
    assert!(
        client
            .download_path
            .starts_with(&format!("{}/", ApiPaths::BRIDGE_DOWNLOAD))
    );
}

#[test]
fn a_floor_is_a_property_of_each_phone_platform() {
    let floors = both().version_floors();
    assert_eq!(
        floors,
        ClientVersionFloors {
            ios: Some(Version::new(1, 2, 0)),
            android: Some(Version::new(1, 3, 0)),
        }
    );
    assert_eq!(
        floors.floor(DevicePlatform::Ios),
        Some(&Version::new(1, 2, 0))
    );
    assert!(floors.admits(DevicePlatform::Ios, &Version::new(1, 2, 0)));
    assert!(!floors.admits(DevicePlatform::Android, &Version::new(1, 2, 0)));
    assert_eq!(
        floors.status(DevicePlatform::Android, "1.2.9"),
        WindowStatus::Below
    );
    assert_eq!(
        floors.status(DevicePlatform::Android, "1.3.0"),
        WindowStatus::Within
    );
    assert_eq!(
        floors.status(DevicePlatform::Ios, "not-a-version"),
        WindowStatus::Unknown
    );
}

#[test]
fn a_platform_with_no_floor_admits_every_version() {
    let floors = both().version_floors();
    for platform in [
        DevicePlatform::Macos,
        DevicePlatform::Windows,
        DevicePlatform::Linux,
        DevicePlatform::Unknown,
    ] {
        assert_eq!(floors.floor(platform), None);
        assert!(floors.admits(platform, &Version::new(0, 0, 1)));
    }
    let open = ClientVersionFloors::default();
    assert!(open.is_open());
    assert!(open.admits(DevicePlatform::Ios, &Version::new(0, 0, 1)));
    assert_eq!(serde_json::to_value(&open).unwrap(), json!({}));
}

#[test]
fn a_prerelease_is_below_the_release_it_precedes() {
    let floors = both().version_floors();
    assert_eq!(
        floors.status(DevicePlatform::Ios, "1.2.0-beta.1"),
        WindowStatus::Below
    );
}
