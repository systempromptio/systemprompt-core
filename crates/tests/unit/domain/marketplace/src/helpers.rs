use systemprompt_identifiers::{MarketplaceId, PluginId};
use systemprompt_models::services::{
    MarketplaceAccess, MarketplaceConfig, MarketplaceVisibility, PluginAuthor, PluginComponentRef,
    PluginConfig, ServicesConfig,
};

#[must_use]
pub fn marketplace(id: &str) -> MarketplaceConfig {
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
        plugins: Default::default(),
        skills: Default::default(),
        mcp_servers: Default::default(),
        agents: Default::default(),
        artifacts: Default::default(),
        access: Default::default(),
    }
}

#[must_use]
pub fn include(values: &[&str]) -> PluginComponentRef {
    PluginComponentRef {
        include: values.iter().map(|v| (*v).to_owned()).collect(),
        ..Default::default()
    }
}

#[must_use]
pub fn access(default_included: bool, roles: &[&str]) -> MarketplaceAccess {
    MarketplaceAccess {
        default_included,
        roles: roles.iter().map(|r| (*r).to_owned()).collect(),
        attributes: Default::default(),
        justification: None,
    }
}

/// Installs a WARN-level subscriber for the duration of the returned guard so
/// the field expressions inside `tracing::warn!` skip/drop branches are
/// evaluated (and therefore counted) rather than short-circuited by the
/// no-subscriber fast path.
#[must_use]
pub fn warn_subscriber_guard() -> tracing::subscriber::DefaultGuard {
    let subscriber = tracing_subscriber::fmt()
        .with_max_level(tracing::Level::WARN)
        .with_test_writer()
        .finish();
    tracing::subscriber::set_default(subscriber)
}

/// An enabled plugin shipping the named artifacts. It also references
/// `skill_id`, because a plugin whose references resolve to no content is
/// dropped from the bundle map — and with it, its artifacts.
#[must_use]
pub fn plugin_shipping_artifacts(id: &str, skill_id: &str, artifact_ids: &[&str]) -> PluginConfig {
    PluginConfig {
        id: PluginId::new(id),
        name: id.to_owned(),
        description: String::new(),
        version: "1.0.0".into(),
        enabled: true,
        author: PluginAuthor {
            name: "test".into(),
            email: "test@example.com".into(),
        },
        keywords: vec![],
        license: "BSL-1.0".into(),
        category: "test".into(),
        skills: include(&[skill_id]),
        agents: Default::default(),
        mcp_servers: Default::default(),
        content_sources: Default::default(),
        artifacts: include(artifact_ids),
        hooks: Default::default(),
        scripts: vec![],
    }
}

/// Writes a minimal enabled skill so a plugin referencing it resolves to real
/// bundle content.
pub fn write_skill_on_disk(root: &std::path::Path, id: &str) {
    let dir = root.join("skills").join(id);
    std::fs::create_dir_all(&dir).expect("create skill dir");
    std::fs::write(
        dir.join("config.yaml"),
        format!("id: {id}\nname: {id}\ndescription: d\nenabled: true\n"),
    )
    .expect("write skill config");
}

#[must_use]
pub fn config_with_plugins(plugins: Vec<PluginConfig>) -> ServicesConfig {
    let mut config = ServicesConfig::default();
    for p in plugins {
        config.plugins.insert(p.id.as_str().to_owned(), p);
    }
    config
}

#[must_use]
pub fn config_with(marketplaces: Vec<MarketplaceConfig>) -> ServicesConfig {
    let mut config = ServicesConfig::default();
    for mp in marketplaces {
        config.marketplaces.insert(mp.id.clone(), mp);
    }
    config
}
