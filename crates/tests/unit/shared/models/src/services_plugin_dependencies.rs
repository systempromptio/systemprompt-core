use std::collections::HashMap;

use systemprompt_identifiers::{MarketplaceId, PluginId};
use systemprompt_models::bridge::plugin_bundle::ManifestDependency;
use systemprompt_models::services::{
    ExternalMarketplace, ExternalMarketplaceSource, MarketplaceConfig, MarketplaceVisibility,
    PluginAuthor, PluginComponentRef, PluginConfig, PluginDependency, ServicesConfig,
};

fn author() -> PluginAuthor {
    PluginAuthor {
        name: "Ed".to_owned(),
        email: "ed@example.com".to_owned(),
    }
}

fn plugin(id: &str, dependencies: Vec<PluginDependency>) -> PluginConfig {
    PluginConfig {
        id: PluginId::new(id),
        name: id.to_owned(),
        description: "d".to_owned(),
        version: "1.0.0".to_owned(),
        enabled: true,
        author: author(),
        keywords: vec![],
        license: "MIT".to_owned(),
        category: "dev".to_owned(),
        skills: PluginComponentRef::default(),
        agents: PluginComponentRef::default(),
        rules: PluginComponentRef::default(),
        mcp_servers: PluginComponentRef::default(),
        content_sources: PluginComponentRef::default(),
        artifacts: PluginComponentRef::default(),
        hooks: Default::default(),
        scripts: vec![],
        dependencies,
    }
}

fn dependency(name: &str, marketplace: Option<&str>, version: Option<&str>) -> PluginDependency {
    PluginDependency {
        name: name.to_owned(),
        marketplace: marketplace.map(str::to_owned),
        version: version.map(str::to_owned),
    }
}

fn marketplace(id: &str, plugins: &[&str]) -> MarketplaceConfig {
    MarketplaceConfig {
        id: MarketplaceId::new(id),
        name: id.to_owned(),
        description: String::new(),
        version: "1.0.0".to_owned(),
        enabled: true,
        author: author(),
        keywords: vec![],
        license: "MIT".to_owned(),
        visibility: MarketplaceVisibility::Public,
        plugins: PluginComponentRef {
            include: plugins.iter().map(|p| (*p).to_owned()).collect(),
            ..Default::default()
        },
        mcp_servers: PluginComponentRef::default(),
        agents: PluginComponentRef::default(),
        artifacts: PluginComponentRef::default(),
        access: Default::default(),
        allow_cross_marketplace_dependencies_on: vec![],
        external_marketplaces: vec![],
    }
}

fn salesforce() -> ExternalMarketplace {
    ExternalMarketplace {
        name: "salesforce".to_owned(),
        source: ExternalMarketplaceSource::Github {
            repo: "SalesforceCommerceCloud/claude-plugins".to_owned(),
        },
    }
}

fn services(plugins: Vec<PluginConfig>, marketplaces: Vec<MarketplaceConfig>) -> ServicesConfig {
    let mut config = ServicesConfig::default();
    config.plugins = plugins
        .into_iter()
        .map(|p| (p.id.as_str().to_owned(), p))
        .collect::<HashMap<_, _>>();
    config.marketplaces = marketplaces
        .into_iter()
        .map(|m| (m.id.clone(), m))
        .collect();
    config
}

#[test]
fn dependency_version_must_be_a_semver_range() {
    let bad = plugin("app", vec![dependency("b2c-cli", None, Some("latest"))]);
    let err = bad.validate("app").expect_err("'latest' is not a range");
    assert!(err.to_string().contains("not a semver range"), "{err}");

    for range in ["^2.0", "~2.1.0", ">=1.4", "=2.1.0", "^2.0.0-0"] {
        let ok = plugin("app", vec![dependency("b2c-cli", None, Some(range))]);
        assert!(ok.validate("app").is_ok(), "{range} is a valid range");
    }
}

#[test]
fn dependency_names_and_marketplaces_must_be_non_empty_and_unique() {
    let unnamed = plugin("app", vec![dependency("  ", None, None)]);
    assert!(unnamed.validate("app").is_err());

    let blank_marketplace = plugin("app", vec![dependency("b2c", Some(""), None)]);
    assert!(blank_marketplace.validate("app").is_err());

    let duplicated = plugin(
        "app",
        vec![
            dependency("b2c", Some("salesforce"), None),
            dependency("b2c", Some("salesforce"), Some("^1")),
        ],
    );
    let err = duplicated.validate("app").expect_err("listed twice");
    assert!(err.to_string().contains("listed twice"), "{err}");
}

#[test]
fn manifest_dependency_uses_claude_code_wire_shapes() {
    let bare = ManifestDependency::from(&dependency("audit-logger", None, None));
    assert_eq!(
        serde_json::to_value(&bare).unwrap(),
        serde_json::json!("audit-logger")
    );

    let full = ManifestDependency::from(&dependency("b2c-cli", Some("salesforce"), Some("^2.0")));
    assert_eq!(
        serde_json::to_value(&full).unwrap(),
        serde_json::json!({ "name": "b2c-cli", "version": "^2.0", "marketplace": "salesforce" })
    );

    let parsed: Vec<ManifestDependency> = serde_json::from_value(serde_json::json!([
        "audit-logger",
        { "name": "b2c", "marketplace": "salesforce" }
    ]))
    .unwrap();
    assert_eq!(parsed[0].name(), "audit-logger");
    assert_eq!(parsed[0].marketplace(), None);
    assert_eq!(parsed[1].marketplace(), Some("salesforce"));
    assert_eq!(PluginDependency::from(&parsed[1]).name, "b2c");
}

#[test]
fn external_marketplace_sources_are_validated_and_serialise_as_claude_code_settings() {
    let mut m = marketplace("org", &[]);
    m.external_marketplaces = vec![ExternalMarketplace {
        name: "bad".to_owned(),
        source: ExternalMarketplaceSource::Github {
            repo: "not-a-repo".to_owned(),
        },
    }];
    assert!(m.validate("org").is_err(), "github repo must be owner/name");

    m.external_marketplaces = vec![ExternalMarketplace {
        name: "bad".to_owned(),
        source: ExternalMarketplaceSource::Git {
            url: "http://example.com/x.git".to_owned(),
        },
    }];
    assert!(m.validate("org").is_err(), "git url must be https");

    m.external_marketplaces = vec![ExternalMarketplace {
        name: "org".to_owned(),
        source: ExternalMarketplaceSource::Github {
            repo: "acme/plugins".to_owned(),
        },
    }];
    assert!(m.validate("org").is_err(), "may not reuse the own id");

    m.external_marketplaces = vec![salesforce()];
    m.allow_cross_marketplace_dependencies_on = vec!["salesforce".to_owned()];
    assert!(m.validate("org").is_ok());
    assert_eq!(
        serde_json::to_value(&m.external_marketplaces[0].source).unwrap(),
        serde_json::json!({ "source": "github", "repo": "SalesforceCommerceCloud/claude-plugins" })
    );
}

#[test]
fn cross_marketplace_dependency_requires_allowlist_and_declaration() {
    let app = plugin("app", vec![dependency("b2c-cli", Some("salesforce"), None)]);

    let unlisted = services(vec![app.clone()], vec![marketplace("org", &["app"])]);
    let err = unlisted.validate().expect_err("target not allowlisted");
    assert!(
        err.to_string()
            .contains("allow_cross_marketplace_dependencies_on"),
        "{err}"
    );

    let mut allowed_only = marketplace("org", &["app"]);
    allowed_only.allow_cross_marketplace_dependencies_on = vec!["salesforce".to_owned()];
    let err = services(vec![app.clone()], vec![allowed_only])
        .validate()
        .expect_err("target not declared");
    assert!(err.to_string().contains("external_marketplaces"), "{err}");

    let mut declared = marketplace("org", &["app"]);
    declared.allow_cross_marketplace_dependencies_on = vec!["salesforce".to_owned()];
    declared.external_marketplaces = vec![salesforce()];
    services(vec![app], vec![declared])
        .validate()
        .expect("allowlisted and declared");
}

#[test]
fn same_marketplace_dependency_must_be_carried() {
    let app = plugin("app", vec![dependency("helper", None, None)]);
    let helper = plugin("helper", vec![]);

    let missing = services(
        vec![app.clone(), helper.clone()],
        vec![marketplace("org", &["app"])],
    );
    let err = missing
        .validate()
        .expect_err("helper is not in the marketplace");
    assert!(err.to_string().contains("does not carry"), "{err}");

    services(
        vec![app, helper],
        vec![marketplace("org", &["app", "helper"])],
    )
    .validate()
    .expect("both carried");
}

#[test]
fn dependency_on_a_sibling_local_marketplace_needs_only_the_allowlist() {
    let app = plugin("app", vec![dependency("shared", Some("platform"), None)]);
    let shared = plugin("shared", vec![]);
    let mut org = marketplace("org", &["app"]);
    org.allow_cross_marketplace_dependencies_on = vec!["platform".to_owned()];
    services(
        vec![app, shared],
        vec![org, marketplace("platform", &["shared"])],
    )
    .validate()
    .expect("a configured marketplace needs no external declaration");
}
