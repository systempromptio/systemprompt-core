use std::collections::BTreeMap;

use systemprompt_identifiers::ModelId;
use systemprompt_security::authz::AuthzContext;

fn floor() -> BTreeMap<String, serde_json::Value> {
    let mut f = BTreeMap::new();
    f.insert(
        "boeing.clearance".to_owned(),
        serde_json::json!(["Internal", "CUI"]),
    );
    f
}

#[test]
fn round_trips_through_none_context() {
    let ctx = AuthzContext::none().with_marketplace_floor(&floor());

    assert!(ctx.is_none(), "kind is preserved across the builder");
    assert_eq!(ctx.marketplace_floor(), Some(floor()));
}

#[test]
fn absent_floor_reads_back_none() {
    assert!(AuthzContext::none().marketplace_floor().is_none());
}

#[test]
fn preserves_typed_payload_alongside_floor() {
    let model = ModelId::new("claude");
    let ctx = AuthzContext::gateway_invocation(&model).with_marketplace_floor(&floor());

    assert_eq!(ctx.marketplace_floor(), Some(floor()));
    assert_eq!(
        ctx.gateway_invocation_model()
            .map(|m| m.as_str().to_owned()),
        Some("claude".to_owned()),
        "floor injection leaves the typed model payload intact",
    );
}

mod resolution {
    use systemprompt_identifiers::MarketplaceId;
    use systemprompt_models::services::{
        MarketplaceConfig, MarketplaceVisibility, PluginAuthor, PluginComponentRef, ServicesConfig,
    };
    use systemprompt_security::authz::member_attribute_floor;
    use systemprompt_security::authz::types::EntityKind;

    fn marketplace(id: &str) -> MarketplaceConfig {
        MarketplaceConfig {
            id: MarketplaceId::new(id),
            name: format!("{id} marketplace"),
            description: String::new(),
            version: "1.0.0".into(),
            enabled: true,
            author: PluginAuthor {
                name: "test".into(),
                email: "test@example.com".into(),
            },
            keywords: vec![],
            license: "BSL-1.0".into(),
            visibility: MarketplaceVisibility::Public,
            plugins: PluginComponentRef::default(),
            mcp_servers: PluginComponentRef::default(),
            agents: PluginComponentRef::default(),
            artifacts: PluginComponentRef::default(),
            access: Default::default(),
        }
    }

    fn include(values: &[&str]) -> PluginComponentRef {
        PluginComponentRef {
            include: values.iter().map(|v| (*v).to_owned()).collect(),
            ..Default::default()
        }
    }

    fn config_with(marketplaces: Vec<MarketplaceConfig>) -> ServicesConfig {
        let mut config = ServicesConfig::default();
        for mp in marketplaces {
            config.marketplaces.insert(mp.id.clone(), mp);
        }
        config
    }

    #[test]
    fn returns_block_for_member() {
        let mut mp = marketplace("market");
        mp.mcp_servers = include(&["sharepoint-sim"]);
        mp.access.attributes.insert(
            "boeing.clearance".to_owned(),
            serde_json::json!(["Internal", "CUI"]),
        );
        let config = config_with(vec![mp]);

        let floor = member_attribute_floor(&config, EntityKind::McpServer, "sharepoint-sim")
            .expect("member inherits the marketplace floor");
        assert_eq!(
            floor.get("boeing.clearance"),
            Some(&serde_json::json!(["Internal", "CUI"]))
        );
    }

    #[test]
    fn covers_every_membership_kind() {
        use systemprompt_identifiers::PluginId;
        use systemprompt_models::services::{ComponentSource, PluginConfig};

        let mut mp = marketplace("market");
        mp.agents = include(&["agent-a"]);
        mp.mcp_servers = include(&["mcp-a"]);
        mp.plugins = include(&["plugin-a"]);
        mp.access
            .attributes
            .insert("tier".to_owned(), serde_json::json!("gold"));
        let mut config = config_with(vec![mp]);
        config.plugins.insert(
            "plugin-a".to_owned(),
            PluginConfig {
                id: PluginId::new("plugin-a"),
                name: "plugin-a".to_owned(),
                description: String::new(),
                version: "1.0.0".to_owned(),
                enabled: true,
                author: PluginAuthor {
                    name: "test".into(),
                    email: "test@example.com".into(),
                },
                keywords: vec![],
                license: "BSL-1.0".to_owned(),
                category: "demo".to_owned(),
                skills: PluginComponentRef {
                    source: ComponentSource::Explicit,
                    include: vec!["skill-a".to_owned()],
                    ..Default::default()
                },
                agents: PluginComponentRef::default(),
                mcp_servers: PluginComponentRef::default(),
                content_sources: PluginComponentRef::default(),
                artifacts: PluginComponentRef::default(),
                hooks: Default::default(),
                scripts: vec![],
            },
        );

        for (kind, id) in [
            (EntityKind::Skill, "skill-a"),
            (EntityKind::Agent, "agent-a"),
            (EntityKind::McpServer, "mcp-a"),
            (EntityKind::Plugin, "plugin-a"),
        ] {
            assert!(
                member_attribute_floor(&config, kind, id).is_some(),
                "{kind:?} member inherits the floor",
            );
        }
        assert!(
            member_attribute_floor(&config, EntityKind::Marketplace, "market").is_none(),
            "kinds without an include list never match",
        );
    }

    #[test]
    fn none_for_non_member() {
        let mut mp = marketplace("market");
        mp.mcp_servers = include(&["sharepoint-sim"]);
        mp.access.attributes.insert(
            "boeing.clearance".to_owned(),
            serde_json::json!(["Internal"]),
        );
        let config = config_with(vec![mp]);

        assert!(member_attribute_floor(&config, EntityKind::McpServer, "other-server").is_none());
    }

    #[test]
    fn none_when_attributes_empty() {
        let mut mp = marketplace("market");
        mp.mcp_servers = include(&["sharepoint-sim"]);
        let config = config_with(vec![mp]);

        assert!(member_attribute_floor(&config, EntityKind::McpServer, "sharepoint-sim").is_none());
    }

    #[test]
    fn none_without_any_marketplace() {
        let config = config_with(vec![]);
        assert!(member_attribute_floor(&config, EntityKind::McpServer, "anything").is_none());
    }

    #[test]
    fn ambiguous_marketplaces_need_an_explicit_default() {
        let mut alpha = marketplace("alpha");
        alpha.mcp_servers = include(&["srv"]);
        alpha
            .access
            .attributes
            .insert("tier".to_owned(), serde_json::json!("gold"));
        let beta = marketplace("beta");
        let mut config = config_with(vec![alpha, beta]);

        assert!(
            member_attribute_floor(&config, EntityKind::McpServer, "srv").is_none(),
            "two marketplaces without a default resolve to no active marketplace",
        );

        config.settings.default_marketplace_id = Some(MarketplaceId::new("alpha"));
        assert!(member_attribute_floor(&config, EntityKind::McpServer, "srv").is_some());
    }
}
