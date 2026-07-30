//! Tests for extension manifest deserialization and `DiscoveredExtension`
//! accessors.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use std::path::PathBuf;

use serde_json::json;
use systemprompt_models::extension::{
    BuildType, DiscoveredExtension, ExtensionManifest, ExtensionType,
};

fn parse(value: serde_json::Value) -> ExtensionManifest {
    serde_json::from_value(value).unwrap()
}

fn minimal() -> serde_json::Value {
    json!({ "extension": { "type": "mcp", "name": "demo" } })
}

fn discovered(value: serde_json::Value) -> DiscoveredExtension {
    DiscoveredExtension::new(
        parse(value),
        PathBuf::from("/srv/ext/demo"),
        PathBuf::from("/srv/ext/demo/manifest.toml"),
    )
}

#[test]
fn minimal_manifest_defaults_enabled_and_workspace_build() {
    let m = parse(minimal());

    assert_eq!(m.extension.name, "demo");
    assert_eq!(m.extension.type_, ExtensionType::Mcp);
    assert!(m.extension.enabled, "enabled defaults to true");
    assert_eq!(m.extension.build_type, BuildType::Workspace);
    assert_eq!(m.extension.description, "");
    assert_eq!(m.extension.binary, None);
    assert_eq!(m.extension.port, None);
    assert!(m.extension.roles.is_empty());
    assert!(m.extension.commands.is_empty());
    assert!(!m.extension.supports_json_output);
}

#[test]
fn extension_type_variants_round_trip_lowercase() {
    for (raw, expected) in [
        ("mcp", ExtensionType::Mcp),
        ("blog", ExtensionType::Blog),
        ("cli", ExtensionType::Cli),
        ("other", ExtensionType::Other),
    ] {
        let m = parse(json!({ "extension": { "type": raw, "name": "x" } }));
        assert_eq!(m.extension.type_, expected);
        assert_eq!(serde_json::to_value(expected).unwrap(), json!(raw));
    }
}

#[test]
fn unknown_extension_type_is_rejected() {
    let err = serde_json::from_value::<ExtensionManifest>(json!({
        "extension": { "type": "wat", "name": "x" }
    }))
    .unwrap_err();
    assert!(
        err.to_string().contains("unknown variant"),
        "unexpected error: {err}"
    );
}

#[test]
fn build_type_submodule_deserializes() {
    let m = parse(json!({
        "extension": { "type": "cli", "name": "x", "build_type": "submodule" }
    }));
    assert_eq!(m.extension.build_type, BuildType::Submodule);
}

#[test]
fn defaults_are_workspace_and_other() {
    assert_eq!(BuildType::default(), BuildType::Workspace);
    assert_eq!(ExtensionType::default(), ExtensionType::Other);
}

#[test]
fn explicit_disable_overrides_default_true() {
    let m = parse(json!({
        "extension": { "type": "cli", "name": "x", "enabled": false }
    }));
    assert!(!m.extension.enabled);
}

#[test]
fn roles_and_commands_parse_with_permission_defaults() {
    let m = parse(json!({
        "extension": {
            "type": "cli",
            "name": "toolbox",
            "binary": "toolbox-bin",
            "port": 7100,
            "supports_json_output": true,
            "commands": [
                { "name": "sync", "description": "Sync everything" },
                { "name": "bare" }
            ],
            "roles": {
                "operator": {
                    "display_name": "Operator",
                    "description": "Runs the toolbox",
                    "permissions": ["toolbox:run", "toolbox:read"]
                },
                "viewer": {
                    "display_name": "Viewer",
                    "description": "Reads only"
                }
            }
        }
    }));

    assert_eq!(m.extension.binary.as_deref(), Some("toolbox-bin"));
    assert_eq!(m.extension.port, Some(7100));
    assert!(m.extension.supports_json_output);

    assert_eq!(m.extension.commands.len(), 2);
    assert_eq!(m.extension.commands[0].name, "sync");
    assert_eq!(m.extension.commands[0].description, "Sync everything");
    assert_eq!(
        m.extension.commands[1].description, "",
        "missing description defaults to empty"
    );

    let operator = &m.extension.roles["operator"];
    assert_eq!(operator.display_name, "Operator");
    assert_eq!(operator.permissions, ["toolbox:run", "toolbox:read"]);
    assert!(
        m.extension.roles["viewer"].permissions.is_empty(),
        "omitted permissions default to empty"
    );
}

#[test]
fn optional_fields_are_skipped_when_serializing() {
    let value = serde_json::to_value(&parse(minimal()).extension).unwrap();
    let obj = value.as_object().unwrap();

    assert!(!obj.contains_key("binary"));
    assert!(!obj.contains_key("port"));
    assert!(!obj.contains_key("roles"));
    assert!(!obj.contains_key("commands"));
    assert_eq!(obj["type"], json!("mcp"));
}

#[test]
fn discovered_accessors_reflect_manifest_for_mcp() {
    let d = discovered(json!({
        "extension": { "type": "mcp", "name": "demo", "binary": "demo-bin" }
    }));

    assert_eq!(d.extension_type(), ExtensionType::Mcp);
    assert_eq!(d.binary_name(), Some("demo-bin"));
    assert!(d.is_enabled());
    assert!(d.is_mcp());
    assert!(!d.is_cli());
    assert!(d.commands().is_empty());
    assert_eq!(d.build_type(), BuildType::Workspace);
    assert!(!d.supports_json_output());
    assert_eq!(d.path, PathBuf::from("/srv/ext/demo"));
    assert_eq!(
        d.manifest_path,
        PathBuf::from("/srv/ext/demo/manifest.toml")
    );
}

#[test]
fn discovered_accessors_reflect_manifest_for_disabled_cli() {
    let d = discovered(json!({
        "extension": {
            "type": "cli",
            "name": "tool",
            "enabled": false,
            "build_type": "submodule",
            "supports_json_output": true,
            "commands": [{ "name": "run" }]
        }
    }));

    assert!(d.is_cli());
    assert!(!d.is_mcp());
    assert!(!d.is_enabled());
    assert_eq!(d.build_type(), BuildType::Submodule);
    assert!(d.supports_json_output());
    assert_eq!(d.binary_name(), None);
    assert_eq!(d.commands().len(), 1);
    assert_eq!(d.commands()[0].name, "run");
}
