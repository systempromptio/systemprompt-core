use std::collections::HashMap;
use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::{
    MarketplaceConfig, MarketplaceVisibility, PluginAuthor, PluginComponentRef, ServicesConfig,
};

fn author() -> PluginAuthor {
    PluginAuthor {
        name: "systemprompt.io".to_string(),
        email: "support@systemprompt.io".to_string(),
    }
}

fn marketplace(id: &str, refs: PluginComponentRef) -> MarketplaceConfig {
    MarketplaceConfig {
        id: MarketplaceId::new(id),
        name: "Test".to_string(),
        description: "Test marketplace".to_string(),
        version: "1.0.0".to_string(),
        enabled: true,
        author: author(),
        keywords: vec![],
        license: "MIT".to_string(),
        visibility: MarketplaceVisibility::Public,
        plugins: refs,
        skills: PluginComponentRef::default(),
        mcp_servers: vec![],
        agents: PluginComponentRef::default(),
    }
}

#[test]
fn marketplace_validate_accepts_kebab_case_id() {
    let m = marketplace("enterprise-demo", PluginComponentRef::default());
    assert!(m.validate("enterprise-demo").is_ok());
}

#[test]
fn marketplace_validate_rejects_short_id() {
    let m = marketplace("ab", PluginComponentRef::default());
    assert!(m.validate("ab").is_err());
}

#[test]
fn marketplace_validate_rejects_uppercase_id() {
    let m = marketplace("Enterprise-Demo", PluginComponentRef::default());
    assert!(m.validate("Enterprise-Demo").is_err());
}

#[test]
fn marketplace_validate_rejects_empty_version() {
    let mut m = marketplace("ok-id", PluginComponentRef::default());
    m.version.clear();
    assert!(m.validate("ok-id").is_err());
}

#[test]
fn services_config_validates_marketplace_with_known_plugin() {
    let mut services = ServicesConfig::default();

    let plugin_id = "demo-plugin".to_string();
    services.plugins.insert(
        plugin_id.clone(),
        systemprompt_models::services::PluginConfig {
            id: systemprompt_identifiers::PluginId::new(&plugin_id),
            name: "Demo".to_string(),
            description: "demo".to_string(),
            version: "1.0.0".to_string(),
            enabled: true,
            author: author(),
            keywords: vec![],
            license: "MIT".to_string(),
            category: "general".to_string(),
            skills: PluginComponentRef::default(),
            agents: PluginComponentRef::default(),
            mcp_servers: vec![],
            content_sources: vec![],
            hooks: Default::default(),
            scripts: vec![],
        },
    );

    let mp_id = MarketplaceId::new("enterprise");
    let mut refs = PluginComponentRef::default();
    refs.include = vec![plugin_id];
    services
        .marketplaces
        .insert(mp_id, marketplace("enterprise", refs));

    services.validate().expect("should validate");
}

#[test]
fn services_config_rejects_marketplace_with_unknown_plugin() {
    let mut services = ServicesConfig::default();
    let mp_id = MarketplaceId::new("enterprise");
    let mut refs = PluginComponentRef::default();
    refs.include = vec!["nonexistent".to_string()];
    services
        .marketplaces
        .insert(mp_id, marketplace("enterprise", refs));

    let err = services.validate().expect_err("should fail");
    assert!(err.to_string().contains("nonexistent"));
}

#[test]
fn services_config_rejects_marketplace_with_unknown_mcp_server() {
    let mut services = ServicesConfig::default();
    let mp_id = MarketplaceId::new("enterprise");
    let mut m = marketplace("enterprise", PluginComponentRef::default());
    m.mcp_servers = vec!["ghost-mcp".to_string()];
    services.marketplaces.insert(mp_id, m);

    let err = services.validate().expect_err("should fail");
    assert!(err.to_string().contains("ghost-mcp"));
}

#[test]
fn services_config_rejects_marketplace_with_unknown_agent() {
    let mut services = ServicesConfig::default();
    let mp_id = MarketplaceId::new("enterprise");
    let mut m = marketplace("enterprise", PluginComponentRef::default());
    m.agents.include = vec!["ghost-agent".to_string()];
    services.marketplaces.insert(mp_id, m);

    let err = services.validate().expect_err("should fail");
    assert!(err.to_string().contains("ghost-agent"));
}

#[test]
fn marketplace_visibility_default_is_public() {
    let v = MarketplaceVisibility::default();
    assert_eq!(v, MarketplaceVisibility::Public);
}

#[test]
fn services_config_has_empty_marketplaces_by_default() {
    let services = ServicesConfig::default();
    assert!(services.marketplaces.is_empty());
}

#[test]
fn marketplace_config_file_deserializes() {
    let yaml = r#"
marketplace:
  id: enterprise-demo
  name: Enterprise Demo
  description: demo
  version: "1.0.0"
  enabled: true
  author:
    name: systemprompt.io
    email: support@systemprompt.io
  license: MIT
  visibility: public
  plugins:
    include: [enterprise-demo]
"#;
    let parsed: systemprompt_models::services::MarketplaceConfigFile =
        serde_yaml::from_str(yaml).expect("should parse");
    assert_eq!(parsed.marketplace.id.as_str(), "enterprise-demo");
    assert_eq!(parsed.marketplace.plugins.include, vec!["enterprise-demo"]);
    assert_eq!(parsed.marketplace.visibility, MarketplaceVisibility::Public);
}

// Silence unused warning for the imports re-used across helpers above.
#[allow(dead_code)]
fn _hint(_: HashMap<String, String>) {}
