//! The `/.well-known/systemprompt-bridge` document: absent without a
//! `gateway.bridge_instance` block, and otherwise the configured routes and
//! branding with the instance-wide enabled hosts the bridge configures. A host
//! that configures itself is never listed, declared or not: the bridge is the
//! document's only reader and manages no such host. The `clients` section
//! lists the phone apps the instance declares and is absent when it declares
//! none.

use systemprompt_api::routes::wellknown::bridge::discovery_document;
use systemprompt_identifiers::ExternalAgentId;
use systemprompt_manifest::services::external_agent::{ExternalAgentConfig, ExternalAgentKind};
use systemprompt_manifest::services::gateway::GatewayState;
use systemprompt_manifest::services::{
    BridgeInstanceSpec, GatewayConfig, GatewayConfigSpec, ServicesConfig,
};
use systemprompt_models::bridge::host::{ConfigOwner, HostKind};
use systemprompt_models::bridge::instance::DISCOVERY_CONTRACT;

fn with_instance(instance: Option<BridgeInstanceSpec>) -> ServicesConfig {
    ServicesConfig {
        gateway: Some(GatewayState::Resolved(GatewayConfig {
            bridge_instance: instance,
            ..GatewayConfig::default()
        })),
        ..ServicesConfig::default()
    }
}

fn acme() -> BridgeInstanceSpec {
    serde_json::from_value(serde_json::json!({
        "device_link_path": "/bridge-auth/device-link",
        "identity_path": "/api/public/bridge/whoami",
        "branding": {
            "display_name": "Acme",
            "docs_url": "https://docs.acme.test",
        },
    }))
    .expect("instance spec parses")
}

fn with_clients(clients: serde_json::Value) -> ServicesConfig {
    let spec: GatewayConfigSpec = serde_json::from_value(serde_json::json!({
        "bridge_instance": {
            "device_link_path": "/bridge-auth/device-link",
            "identity_path": "/api/public/bridge/whoami",
            "clients": clients,
        }
    }))
    .expect("gateway spec parses");
    ServicesConfig {
        gateway: Some(GatewayState::Resolved(spec.resolve())),
        ..ServicesConfig::default()
    }
}

#[test]
fn the_document_of_an_instance_with_no_apps_has_no_clients_section() {
    let document = discovery_document(&with_instance(Some(acme()))).expect("configured");
    assert!(document.clients.is_empty());
    let json = serde_json::to_value(&document).expect("serialise");
    assert!(json.get("clients").is_none(), "{json}");
}

#[test]
fn the_document_lists_each_configured_app_with_its_floor_and_where_to_get_it() {
    let document = discovery_document(&with_clients(serde_json::json!({
        "ios": {
            "app_id": "io.example.app",
            "min_version": "1.2.0",
            "link": "https://apps.example.com/redeem",
        },
        "android": { "app_id": "io.example.android", "min_version": "1.3.0" },
    })))
    .expect("configured");
    let json = serde_json::to_value(&document).expect("serialise");
    assert_eq!(
        json["clients"],
        serde_json::json!({
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
    let floors = document.clients.version_floors();
    assert_eq!(floors.ios, Some(semver::Version::new(1, 2, 0)));
    assert_eq!(floors.android, Some(semver::Version::new(1, 3, 0)));
}

#[test]
fn the_document_lists_only_the_platform_that_has_an_app() {
    let document = discovery_document(&with_clients(serde_json::json!({
        "android": { "app_id": "io.example.android" },
    })))
    .expect("configured");
    assert_eq!(
        serde_json::to_value(&document).expect("serialise")["clients"],
        serde_json::json!({
            "android": {
                "app_id": "io.example.android",
                "download_path": "/v1/bridge/download/android",
            },
        })
    );
}

#[test]
fn no_document_without_a_gateway_section() {
    assert!(discovery_document(&ServicesConfig::default()).is_none());
}

#[test]
fn no_document_without_a_bridge_instance_block() {
    assert!(discovery_document(&with_instance(None)).is_none());
}

#[test]
fn the_document_carries_the_configured_routes_and_branding() {
    let document = discovery_document(&with_instance(Some(acme()))).expect("configured");
    assert_eq!(document.contract, DISCOVERY_CONTRACT);
    assert_eq!(document.device_link_path, "/bridge-auth/device-link");
    assert_eq!(document.identity_path, "/api/public/bridge/whoami");
    assert_eq!(document.branding.display_name.as_deref(), Some("Acme"));
    assert_eq!(
        document.branding.docs_url.as_deref(),
        Some("https://docs.acme.test")
    );
    assert!(document.branding.logo_url.is_none());
    assert_eq!(document.enabled_hosts, bridge_configured());
}

fn bridge_configured() -> Vec<HostKind> {
    HostKind::ALL
        .into_iter()
        .filter(|host| host.config_owner() == ConfigOwner::Bridge)
        .collect()
}

fn agent(id: &str, enabled: bool) -> (ExternalAgentId, ExternalAgentConfig) {
    let id = ExternalAgentId::try_new(id).expect("valid id");
    (
        id.clone(),
        ExternalAgentConfig {
            id,
            display_name: "Agent".to_owned(),
            kind: ExternalAgentKind::DesktopApp,
            enabled,
            default_enabled: true,
            description: String::new(),
            platforms: Vec::new(),
            docs_url: None,
            desktop_policy: systemprompt_models::bridge::desktop_policy::DesktopPolicy::default(),
            extensions: Vec::new(),
        },
    )
}

#[test]
fn the_document_never_lists_a_host_that_configures_itself() {
    let default_document = discovery_document(&with_instance(Some(acme()))).expect("configured");
    assert!(!default_document.enabled_hosts.contains(&HostKind::Harness));

    let mut services = with_instance(Some(acme()));
    services.external_agents.extend([agent("harness", true)]);
    assert!(
        services
            .instance_enabled_hosts()
            .contains(&HostKind::Harness),
        "the instance serves harness"
    );
    let document = discovery_document(&services).expect("configured");
    assert_eq!(
        document.enabled_hosts,
        bridge_configured(),
        "a bridge of any version parses the document of an instance that enables harness"
    );
    let wire = serde_json::to_string(&document).expect("serialise");
    assert!(!wire.contains("harness"), "{wire}");
}

#[test]
fn the_document_omits_hosts_the_instance_disabled() {
    let mut services = with_instance(Some(acme()));
    let id = ExternalAgentId::try_new("codex_cli").expect("valid id");
    services.external_agents.insert(
        id.clone(),
        ExternalAgentConfig {
            id,
            display_name: "Codex CLI".to_owned(),
            kind: ExternalAgentKind::CliTool,
            enabled: false,
            default_enabled: true,
            description: String::new(),
            platforms: Vec::new(),
            docs_url: None,
            desktop_policy: systemprompt_models::bridge::desktop_policy::DesktopPolicy::default(),
            extensions: Vec::new(),
        },
    );
    let document = discovery_document(&services).expect("configured");
    assert!(!document.enabled_hosts.contains(&HostKind::CodexCli));
}

#[test]
fn the_document_wire_shape_has_no_trust_material() {
    let document = discovery_document(&with_instance(Some(acme()))).expect("configured");
    let json = serde_json::to_value(&document).expect("serialise");
    let mut keys: Vec<&str> = json
        .as_object()
        .expect("object")
        .keys()
        .map(String::as_str)
        .collect();
    keys.sort_unstable();
    assert_eq!(
        keys,
        [
            "branding",
            "contract",
            "device_link_path",
            "enabled_hosts",
            "identity_path"
        ]
    );
}
