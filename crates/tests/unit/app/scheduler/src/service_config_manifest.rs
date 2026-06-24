//! Tests for [`ServiceConfig::list_from_manifest`], the projection of a loaded
//! `ServicesConfig` into the flat agent + MCP service list the reconciler's
//! state verifier consumes.

use systemprompt_models::ServicesConfig;
use systemprompt_scheduler::{ServiceConfig, ServiceType};

fn empty_manifest() -> ServicesConfig {
    ServicesConfig::default()
}

#[test]
fn empty_manifest_yields_empty_list() {
    let list = ServiceConfig::list_from_manifest(&empty_manifest());
    assert!(list.is_empty());
}

#[test]
fn single_mcp_server_is_projected_as_mcp_service() {
    // A minimal internal MCP deployment; only the fields the projection reads
    // (port, enabled) carry observable assertions.
    let json = r#"{
        "mcp_servers": {
            "echo": {
                "binary": "echo-server",
                "port": 3010,
                "enabled": true,
                "display_in_web": false,
                "oauth": {
                    "required": false,
                    "scopes": [],
                    "audience": "Mcp",
                    "client_id": null
                }
            }
        }
    }"#;

    let manifest: ServicesConfig =
        serde_json::from_str(json).expect("minimal mcp services manifest should deserialize");

    let list = ServiceConfig::list_from_manifest(&manifest);
    assert_eq!(list.len(), 1);

    let svc = &list[0];
    assert_eq!(svc.name, "echo");
    assert_eq!(svc.service_type, ServiceType::Mcp);
    assert_eq!(svc.port, 3010);
    assert!(svc.enabled);
}

#[test]
fn disabled_mcp_server_keeps_disabled_flag() {
    let json = r#"{
        "mcp_servers": {
            "disabled_one": {
                "binary": "x",
                "port": 3099,
                "enabled": false,
                "display_in_web": false,
                "oauth": {
                    "required": false,
                    "scopes": [],
                    "audience": "Mcp",
                    "client_id": null
                }
            }
        }
    }"#;

    let manifest: ServicesConfig =
        serde_json::from_str(json).expect("disabled mcp manifest should deserialize");

    let list = ServiceConfig::list_from_manifest(&manifest);
    assert_eq!(list.len(), 1);
    assert!(!list[0].enabled);
    assert_eq!(list[0].service_type, ServiceType::Mcp);
}

#[test]
fn multiple_mcp_servers_are_all_projected() {
    let json = r#"{
        "mcp_servers": {
            "a": {
                "binary": "a",
                "port": 3001,
                "enabled": true,
                "display_in_web": false,
                "oauth": { "required": false, "scopes": [], "audience": "Mcp", "client_id": null }
            },
            "b": {
                "binary": "b",
                "port": 3002,
                "enabled": true,
                "display_in_web": false,
                "oauth": { "required": false, "scopes": [], "audience": "Mcp", "client_id": null }
            }
        }
    }"#;

    let manifest: ServicesConfig =
        serde_json::from_str(json).expect("multi mcp manifest should deserialize");

    let list = ServiceConfig::list_from_manifest(&manifest);
    assert_eq!(list.len(), 2);
    assert!(list.iter().all(|s| s.service_type == ServiceType::Mcp));

    let mut ports: Vec<u16> = list.iter().map(|s| s.port).collect();
    ports.sort_unstable();
    assert_eq!(ports, vec![3001, 3002]);
}

#[test]
fn external_mcp_server_is_excluded() {
    let json = r#"{
        "mcp_servers": {
            "remote_one": {
                "binary": "",
                "server_type": "external",
                "endpoint": "https://example.com/mcp",
                "port": 0,
                "enabled": true,
                "display_in_web": false,
                "oauth": { "required": false, "scopes": [], "audience": "Mcp", "client_id": null }
            }
        }
    }"#;

    let manifest: ServicesConfig =
        serde_json::from_str(json).expect("external mcp manifest should deserialize");

    let list = ServiceConfig::list_from_manifest(&manifest);
    assert!(
        list.is_empty(),
        "external MCP servers are not subprocess-managed and must be excluded"
    );
}

#[test]
fn external_mcp_excluded_while_internal_retained() {
    let json = r#"{
        "mcp_servers": {
            "local": {
                "binary": "local-server",
                "server_type": "internal",
                "port": 3010,
                "enabled": true,
                "display_in_web": false,
                "oauth": { "required": false, "scopes": [], "audience": "Mcp", "client_id": null }
            },
            "remote": {
                "binary": "",
                "server_type": "external",
                "endpoint": "https://example.com/mcp",
                "port": 0,
                "enabled": true,
                "display_in_web": false,
                "oauth": { "required": false, "scopes": [], "audience": "Mcp", "client_id": null }
            }
        }
    }"#;

    let manifest: ServicesConfig =
        serde_json::from_str(json).expect("mixed mcp manifest should deserialize");

    let list = ServiceConfig::list_from_manifest(&manifest);
    assert_eq!(list.len(), 1);
    assert_eq!(list[0].name, "local");
    assert_eq!(list[0].service_type, ServiceType::Mcp);
}
