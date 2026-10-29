//! The `/.well-known/systemprompt-bridge` document: absent without a
//! `gateway.bridge_instance` block, and otherwise the configured routes and
//! branding with the instance-wide enabled hosts.

use systemprompt_api::routes::wellknown::bridge::discovery_document;
use systemprompt_identifiers::ExternalAgentId;
use systemprompt_manifest::services::external_agent::{ExternalAgentConfig, ExternalAgentKind};
use systemprompt_manifest::services::gateway::GatewayState;
use systemprompt_manifest::services::{
    BridgeBrandingSpec, BridgeInstanceSpec, GatewayConfig, ServicesConfig,
};
use systemprompt_models::bridge::host::HostKind;
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
    BridgeInstanceSpec {
        device_link_path: "/bridge-auth/device-link".to_owned(),
        identity_path: "/api/public/bridge/whoami".to_owned(),
        branding: BridgeBrandingSpec {
            display_name: Some("Acme".to_owned()),
            docs_url: Some("https://docs.acme.test".to_owned()),
            ..BridgeBrandingSpec::default()
        },
    }
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
    assert_eq!(document.enabled_hosts, HostKind::ALL.to_vec());
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
