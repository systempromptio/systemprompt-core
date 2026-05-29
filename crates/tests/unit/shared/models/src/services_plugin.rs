use systemprompt_identifiers::{MarketplaceId, PluginId};
use systemprompt_models::services::{
    ComponentFilter, ComponentSource, MarketplaceConfig, MarketplaceVisibility, McpServerSummary,
    PluginAuthor, PluginComponentRef, PluginConfig, PluginScript, PluginSummary, PluginVariableDef,
};

fn author() -> PluginAuthor {
    PluginAuthor {
        name: "Ed".to_owned(),
        email: "ed@example.com".to_owned(),
    }
}

fn valid_plugin(id: &str) -> PluginConfig {
    PluginConfig {
        id: PluginId::new(id),
        name: "Plugin".to_owned(),
        description: "d".to_owned(),
        version: "1.0.0".to_owned(),
        enabled: true,
        author: author(),
        keywords: vec!["k".to_owned()],
        license: "MIT".to_owned(),
        category: "dev".to_owned(),
        skills: PluginComponentRef::default(),
        agents: PluginComponentRef::default(),
        mcp_servers: PluginComponentRef::default(),
        content_sources: PluginComponentRef::default(),
        scripts: vec![],
    }
}

#[test]
fn component_source_display_and_default() {
    assert_eq!(ComponentSource::default(), ComponentSource::Instance);
    assert_eq!(ComponentSource::Instance.to_string(), "instance");
    assert_eq!(ComponentSource::Explicit.to_string(), "explicit");
}

#[test]
fn component_filter_display() {
    assert_eq!(ComponentFilter::Enabled.to_string(), "enabled");
}

#[test]
fn component_source_serde_lowercase() {
    let json = serde_json::to_string(&ComponentSource::Explicit).unwrap();
    assert_eq!(json, "\"explicit\"");
    let parsed: ComponentSource = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed, ComponentSource::Explicit);
}

#[test]
fn plugin_summary_from_config_counts_includes() {
    let mut p = valid_plugin("my-plugin");
    p.skills.include = vec!["s1".to_owned(), "s2".to_owned()];
    p.agents.include = vec!["a1".to_owned()];
    let summary: PluginSummary = (&p).into();
    assert_eq!(summary.id, p.id);
    assert_eq!(summary.skill_count, 2);
    assert_eq!(summary.agent_count, 1);
    assert!(summary.enabled);
}

#[test]
fn plugin_validate_accepts_kebab_case_ids() {
    let p = valid_plugin("a-good-id-123");
    assert!(p.validate("key").is_ok());
}

#[test]
fn plugin_validate_rejects_short_id() {
    let p = valid_plugin("ab");
    let err = p.validate("k").unwrap_err();
    assert!(format!("{err}").contains("between 3 and 50"));
}

#[test]
fn plugin_validate_rejects_long_id() {
    let p = valid_plugin(&"a".repeat(51));
    let err = p.validate("k").unwrap_err();
    assert!(format!("{err}").contains("between 3 and 50"));
}

#[test]
fn plugin_validate_rejects_uppercase_id() {
    let p = valid_plugin("MyPlugin");
    let err = p.validate("k").unwrap_err();
    assert!(format!("{err}").contains("kebab-case"));
}

#[test]
fn plugin_validate_rejects_underscore_id() {
    let p = valid_plugin("my_plugin");
    let err = p.validate("k").unwrap_err();
    assert!(format!("{err}").contains("kebab-case"));
}

#[test]
fn plugin_validate_rejects_empty_version() {
    let mut p = valid_plugin("good-id");
    p.version = String::new();
    let err = p.validate("k").unwrap_err();
    assert!(format!("{err}").contains("version"));
}

#[test]
fn plugin_validate_rejects_explicit_source_with_empty_include() {
    let mut p = valid_plugin("good-id");
    p.skills.source = ComponentSource::Explicit;
    p.skills.include = vec![];
    let err = p.validate("k").unwrap_err();
    assert!(format!("{err}").contains("explicit"));
}

#[test]
fn plugin_validate_accepts_explicit_source_with_include() {
    let mut p = valid_plugin("good-id");
    p.agents.source = ComponentSource::Explicit;
    p.agents.include = vec!["agent-1".to_owned()];
    assert!(p.validate("k").is_ok());
}

#[test]
fn plugin_variable_def_default_required_is_true_via_serde() {
    let json = r#"{"name":"FOO"}"#;
    let v: PluginVariableDef = serde_json::from_str(json).unwrap();
    assert!(v.required);
    assert!(!v.secret);
    assert!(v.example.is_none());
    assert!(v.description.is_empty());
}

#[test]
fn plugin_script_round_trips_serde() {
    let s = PluginScript {
        name: "init".to_owned(),
        source: "echo hi".to_owned(),
    };
    let json = serde_json::to_string(&s).unwrap();
    let parsed: PluginScript = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "init");
}

#[test]
fn marketplace_visibility_default_and_serde() {
    assert_eq!(
        MarketplaceVisibility::default(),
        MarketplaceVisibility::Public
    );
    let json = serde_json::to_string(&MarketplaceVisibility::Org).unwrap();
    assert_eq!(json, "\"org\"");
    let parsed: MarketplaceVisibility = serde_json::from_str("\"private\"").unwrap();
    assert_eq!(parsed, MarketplaceVisibility::Private);
}

fn valid_marketplace(id: &str) -> MarketplaceConfig {
    MarketplaceConfig {
        id: MarketplaceId::new(id),
        name: "Market".to_owned(),
        description: "desc".to_owned(),
        version: "1.0.0".to_owned(),
        enabled: true,
        author: author(),
        keywords: vec![],
        license: "MIT".to_owned(),
        visibility: MarketplaceVisibility::Public,
        plugins: PluginComponentRef::default(),
        skills: PluginComponentRef::default(),
        mcp_servers: PluginComponentRef::default(),
        agents: PluginComponentRef::default(),
        access: Default::default(),
    }
}

#[test]
fn marketplace_validate_passes_for_well_formed() {
    let m = valid_marketplace("market-1");
    assert!(m.validate("k").is_ok());
}

#[test]
fn marketplace_validate_rejects_id_length() {
    let m = valid_marketplace("ab");
    assert!(m.validate("k").is_err());
    let m = valid_marketplace(&"a".repeat(51));
    assert!(m.validate("k").is_err());
}

#[test]
fn marketplace_validate_rejects_non_kebab_id() {
    let m = valid_marketplace("Market_1");
    let err = m.validate("k").unwrap_err();
    assert!(format!("{err}").contains("kebab-case"));
}

#[test]
fn marketplace_validate_rejects_empty_version() {
    let mut m = valid_marketplace("market-1");
    m.version = String::new();
    let err = m.validate("k").unwrap_err();
    assert!(format!("{err}").contains("version"));
}

#[test]
fn mcp_server_summary_serde_round_trip() {
    let s = McpServerSummary {
        name: "fs".to_owned(),
        display_name: "Filesystem".to_owned(),
        enabled: true,
        port: 5050,
        status: Some("running".to_owned()),
        binary_debug: None,
        binary_release: Some("/bin/fs".to_owned()),
        debug_created_at: None,
        release_created_at: None,
        created_at: Some("2025-01-01".to_owned()),
    };
    let json = serde_json::to_string(&s).unwrap();
    assert!(json.contains("\"status\":\"running\""));
    assert!(!json.contains("binary_debug"));
    let parsed: McpServerSummary = serde_json::from_str(&json).unwrap();
    assert_eq!(parsed.name, "fs");
    assert_eq!(parsed.port, 5050);
}
