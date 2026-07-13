use systemprompt_identifiers::MarketplaceId;
use systemprompt_models::services::{
    MarketplaceAccess, MarketplaceConfig, MarketplaceVisibility, PluginAuthor, PluginComponentRef,
    ServicesConfig,
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

#[must_use]
pub fn config_with(marketplaces: Vec<MarketplaceConfig>) -> ServicesConfig {
    let mut config = ServicesConfig::default();
    for mp in marketplaces {
        config.marketplaces.insert(mp.id.clone(), mp);
    }
    config
}
