use systemprompt_device::protocol::jcs::{is_canonical, to_canonical};
use systemprompt_device::protocol::manifest::{Manifest, probe_schema_version};
use systemprompt_device::protocol::targets::{enables_host, entry_targets_host, rule_targets_host};
use systemprompt_models::bridge::host::HostKind;
use systemprompt_models::bridge::ids::ToolPolicy;
use systemprompt_models::bridge::manifest::{SignedManifestEnvelope, ThemeMode};

const D1: &str = "3322c4ddc2a08588fa10168039bf874590bf50cb59d7b03575ac042cc70ead90";
const D2: &str = "b3bb6c15ade34221806687bae4d2ff5ccaaf56ab24911bc7859d88dd8c3b9fa4";

fn parse(value: &serde_json::Value) -> Result<Manifest, serde_json::Error> {
    Manifest::parse(&value.to_string())
}

fn agents() -> serde_json::Value {
    serde_json::json!([{
        "description": "Local test agent of the alpha instance",
        "display_name": "alpha helper",
        "enabled": true,
        "endpoint": "http://localhost:8080/api/v1/agents/alpha_helper/",
        "id": "alpha_helper",
        "is_default": false,
        "is_primary": false,
        "mcp_servers": { "include": ["alpha-docs"], "source": "explicit" },
        "model": "claude-sonnet-5-5",
        "name": "alpha_helper",
        "provider": "anthropic",
        "skills": { "include": ["alpha_notes"], "source": "explicit" },
        "system_prompt": "You are the alpha helper.",
        "tags": [],
        "version": "1.0.0"
    }])
}

fn mcp_servers() -> serde_json::Value {
    serde_json::json!([{
        "id": "alpha-docs",
        "name": "alpha-docs",
        "oauth": false,
        "tool_policy": { "*": "allow", "delete": "deny" },
        "transport": "http",
        "url": "http://127.0.0.1:9/alpha-docs/mcp"
    }])
}

fn marketplaces() -> serde_json::Value {
    serde_json::json!([
        { "id": "alpha-local", "name": "alpha-local", "plugin_ids": ["alpha-tools"] },
        {
            "id": "astound-commons",
            "name": "Astound Digital — Commons",
            "plugin_ids": ["astound-commons"],
            "harness": {
                "defaults": { "model": "systemprompt/claude-sonnet-5-5", "share": "disabled" },
                "themes": [{ "name": "astound", "sha": D1 }],
                "default_theme": "astound",
                "theme_mode": "dark"
            }
        }
    ])
}

fn plugins() -> serde_json::Value {
    serde_json::json!([
        {
            "files": [{ "path": ".claude-plugin/plugin.json", "sha256": D1, "size": 233 }],
            "hooks": { "comms": false, "governance": false, "judge": false },
            "id": "alpha-tools",
            "sha256": D2,
            "version": "1.0.0"
        },
        {
            "files": [{ "path": "skills/who-am-i/SKILL.md", "sha256": D2, "size": 2622 }],
            "hooks": { "comms": false, "governance": true, "judge": true },
            "id": "astound-commons",
            "sha256": D1,
            "version": "1.0.0"
        }
    ])
}

fn rules() -> serde_json::Value {
    serde_json::json!([{
        "description": "",
        "file_path": "/app/services/rules/alpha_conventions/index.md",
        "id": "alpha_conventions",
        "instructions": "# Alpha conventions",
        "name": "alpha conventions",
        "plugins": ["alpha-tools"],
        "sha256": D1,
        "tags": [],
        "hosts": ["harness"]
    }])
}

fn skills() -> serde_json::Value {
    serde_json::json!([{
        "description": "Turn a working session into short alpha-team notes.",
        "file_path": "managed://alpha_notes@b3bb",
        "id": "alpha_notes",
        "instructions": "# Alpha notes",
        "name": "alpha notes",
        "plugins": ["alpha-tools"],
        "publication": {
            "bundle_digest": D2,
            "generation": 1,
            "publication_id": "750981ce-807d-4c97-8607-0d510dd51a63",
            "resource_id": "1d1ee2f7-5968-48c6-abc6-2c993721f681",
            "revision_id": "802c1429-27ed-4274-afd9-51e2ed79ea79"
        },
        "sha256": D1,
        "tags": []
    }])
}

fn fixture() -> serde_json::Value {
    serde_json::json!({
        "agents": agents(),
        "allow_claude_ai_connectors": false,
        "allow_user_disable": false,
        "artifacts": [],
        "auto_update": "staged",
        "branding": { "display_name": "Astound Digital" },
        "desktop_policy": { "schema_version": 1, "settings": { "toolSearchEnabled": true } },
        "enabled_hosts": ["claude-code", "claude-desktop", "harness"],
        "hooks": [],
        "host_model_protocols": { "harness": ["openai"] },
        "issued_at": "2026-11-01T09:12:31.255641757Z",
        "managed_mcp_servers": mcp_servers(),
        "manifest_version": "2026-11-01T09:12:31Z-000001a19661c117",
        "marketplaces": marketplaces(),
        "min_bridge_version": "0.28.0",
        "min_schema_version": 1,
        "not_before": "2026-11-01T09:11:31.255641757Z",
        "plugins": plugins(),
        "revocations": ["4e261575-c9fe-4781-91a5-f395f2cf2f51"],
        "rules": rules(),
        "skills": skills(),
        "tenant_id": null,
        "user": {
            "display_name": "Dev User",
            "email": "dev@astound.test",
            "id": "1ecce4e3-bc87-441d-b374-ea29bab88da8",
            "name": "dev",
            "roles": ["user"]
        },
        "user_id": "1ecce4e3-bc87-441d-b374-ea29bab88da8"
    })
}

#[test]
fn decodes_the_alpha_fixture() {
    let manifest = parse(&fixture()).unwrap();
    assert!(!manifest.schema_too_new());
    assert_eq!(
        manifest.user_id.as_str(),
        "1ecce4e3-bc87-441d-b374-ea29bab88da8"
    );
    assert_eq!(
        manifest.manifest_version.as_str(),
        "2026-11-01T09:12:31Z-000001a19661c117"
    );
    assert_eq!(
        manifest.skills[0]
            .publication
            .as_ref()
            .map(|p| p.generation),
        Some(1)
    );
    assert_eq!(
        manifest.governance_plugin().map(|p| p.id.as_str()),
        Some("astound-commons")
    );
    assert_eq!(
        manifest.managed_mcp_servers[0].policy_for_tool("delete"),
        Some(ToolPolicy::Deny)
    );
    assert_eq!(
        manifest.managed_mcp_servers[0].policy_for_tool("read"),
        Some(ToolPolicy::Allow)
    );
    assert_eq!(
        manifest.model_families(HostKind::Harness),
        Some(&["openai".to_owned()][..])
    );
    assert_eq!(
        manifest.user.as_ref().map(|u| u.roles.clone()),
        Some(vec!["user".to_owned()])
    );
}

#[test]
fn host_targeting_on_the_fixture() {
    let manifest = parse(&fixture()).unwrap();
    assert!(enables_host(&manifest.enabled_hosts, HostKind::Harness));
    assert!(entry_targets_host(
        &manifest.skills[0].hosts,
        HostKind::Harness
    ));
    assert!(rule_targets_host(
        &manifest.rules[0].hosts,
        HostKind::Harness
    ));
}

#[test]
fn reads_the_host_settings_block() {
    let manifest = parse(&fixture()).unwrap();
    let settings = manifest.host_settings().unwrap();
    assert_eq!(
        settings.default_model(),
        Some(("systemprompt", "claude-sonnet-5-5"))
    );
    assert_eq!(
        settings.default_theme_ref().map(|t| t.name.as_str()),
        Some("astound")
    );
    assert_eq!(settings.theme_mode, Some(ThemeMode::Dark));
}

#[test]
fn absent_lists_default_and_unknown_hosts_do_not_fail() {
    let mut value = serde_json::json!({
        "user_id": "u-1",
        "manifest_version": "2026-11-01T09:12:31Z-000001a19661c117",
        "issued_at": "2026-11-01T09:12:31Z",
        "not_before": "2026-11-01T09:11:31Z",
        "revocations": [],
        "managed_mcp_servers": [{ "name": "docs", "url": "https://sp.example.com/mcp" }],
        "plugins": [],
        "enabled_hosts": ["claude-code"]
    });
    let manifest = parse(&value).unwrap();
    assert!(
        manifest.rules.is_empty() && manifest.skills.is_empty() && manifest.marketplaces.is_empty()
    );
    assert_eq!(manifest.managed_mcp_servers[0].id.as_str(), "docs");
    assert!(!enables_host(&manifest.enabled_hosts, HostKind::Harness));
    assert!(manifest.host_settings().is_none());
    assert!(manifest.governance_plugin().is_none());
    value["enabled_hosts"] = serde_json::json!(["some-future-host"]);
    let manifest = parse(&value).unwrap();
    assert!(!enables_host(&manifest.enabled_hosts, HostKind::Harness));
}

#[test]
fn a_bad_digest_or_user_fails_the_manifest() {
    let mut value = fixture();
    value["plugins"][0]["sha256"] = serde_json::json!("ABC");
    assert!(parse(&value).is_err());
    let mut value = fixture();
    value["user_id"] = serde_json::json!("has space");
    assert!(parse(&value).is_err());
}

#[test]
fn round_trips_through_jcs() {
    let manifest = parse(&fixture()).unwrap();
    let canonical = to_canonical(manifest.signed()).unwrap();
    assert!(is_canonical(&canonical));
    let back = Manifest::parse(&canonical).unwrap();
    assert_eq!(to_canonical(back.signed()).unwrap(), canonical);
}

#[test]
fn schema_probe() {
    assert_eq!(
        probe_schema_version(r#"{"min_schema_version":3,"user_id":5}"#),
        Some(3)
    );
    assert_eq!(probe_schema_version(r#"{"user_id":"u"}"#), None);
    let too_new = parse(&serde_json::json!({
        "min_schema_version": 2, "user_id": "u", "managed_mcp_servers": [], "plugins": [],
        "manifest_version": "2026-11-01T09:12:31Z-000001a19661c117",
        "issued_at": "2026-11-01T09:12:31Z", "not_before": "2026-11-01T09:11:31Z",
        "revocations": []
    }))
    .unwrap();
    assert!(too_new.schema_too_new());
}

#[test]
fn the_envelope_decodes() {
    let envelope: SignedManifestEnvelope =
        serde_json::from_str(r#"{"payload":"{\"a\":1}","signature":"c2ln"}"#).unwrap();
    assert_eq!(envelope.payload, r#"{"a":1}"#);
    assert_eq!(envelope.signature.as_str(), "c2ln");
}
