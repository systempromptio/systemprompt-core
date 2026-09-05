use systemprompt_identifiers::MarketplaceId;
use systemprompt_marketplace::{MarketplaceError, MarketplaceService};

use crate::helpers::{config_with, include, marketplace};

#[test]
fn resolve_default_uses_explicit_id() {
    let mut config = config_with(vec![marketplace("primary"), marketplace("secondary")]);
    config.settings.default_marketplace_id = Some(MarketplaceId::new("secondary"));
    let service = MarketplaceService::new(&config);

    let (id, mp) = service
        .resolve_default()
        .expect("explicit default resolves");
    assert_eq!(id.as_str(), "secondary");
    assert_eq!(mp.id.as_str(), "secondary");
}

#[test]
fn resolve_default_uses_sole_marketplace_without_explicit_id() {
    let config = config_with(vec![marketplace("only")]);
    let service = MarketplaceService::new(&config);

    let (id, mp) = service
        .resolve_default()
        .expect("the single configured marketplace resolves");
    assert_eq!(id.as_str(), "only");
    assert_eq!(mp.id.as_str(), "only");
}

#[test]
fn resolve_default_errors_when_none() {
    let config = config_with(vec![marketplace("alpha"), marketplace("beta")]);
    let service = MarketplaceService::new(&config);

    assert!(matches!(
        service.resolve_default(),
        Err(MarketplaceError::NoDefault)
    ));
}

#[test]
fn get_hit_returns_config() {
    let config = config_with(vec![marketplace("alpha")]);
    let service = MarketplaceService::new(&config);

    let mp = service
        .get(&MarketplaceId::new("alpha"))
        .expect("existing marketplace is found");
    assert_eq!(mp.id.as_str(), "alpha");
}

#[test]
fn get_miss_returns_not_found() {
    let config = config_with(vec![marketplace("alpha")]);
    let service = MarketplaceService::new(&config);

    assert!(matches!(
        service.get(&MarketplaceId::new("missing")),
        Err(MarketplaceError::NotFound(_))
    ));
}

#[test]
fn enabled_is_empty_without_any_marketplace() {
    let config = config_with(vec![]);
    assert!(MarketplaceService::new(&config).enabled().is_empty());
}

#[test]
fn enabled_lists_every_enabled_marketplace_sorted() {
    let config = config_with(vec![marketplace("beta"), marketplace("alpha")]);
    let service = MarketplaceService::new(&config);
    let ids: Vec<&str> = service
        .enabled()
        .iter()
        .map(|(id, _)| id.as_str())
        .collect();
    assert_eq!(ids, vec!["alpha", "beta"]);
}

#[test]
fn resolve_default_selects_the_named_marketplace_when_many() {
    let mut config = config_with(vec![marketplace("alpha"), marketplace("beta")]);
    config.settings.default_marketplace_id = Some(MarketplaceId::new("beta"));
    let service = MarketplaceService::new(&config);

    let (_, default) = service
        .resolve_default()
        .expect("default names the rendering marketplace");
    assert_eq!(default.id.as_str(), "beta");
}

#[test]
fn resolve_default_fails_when_many_and_none_is_named() {
    let config = config_with(vec![marketplace("alpha"), marketplace("beta")]);
    assert!(
        MarketplaceService::new(&config).resolve_default().is_err(),
        "the rendered marketplace.json still needs one named marketplace",
    );
}

#[test]
fn validate_referential_integrity_passes_for_consistent_config() {
    let config = config_with(vec![marketplace("solo")]);
    let service = MarketplaceService::new(&config);
    service
        .validate_referential_integrity()
        .expect("a self-consistent services config validates");
}

#[test]
fn validate_referential_integrity_flags_dangling_reference() {
    let mut mp = marketplace("market");
    mp.plugins = include(&["never-defined-plugin"]);
    let config = config_with(vec![mp]);
    let service = MarketplaceService::new(&config);

    assert!(
        matches!(
            service.validate_referential_integrity(),
            Err(MarketplaceError::Validation(_))
        ),
        "a marketplace referencing an undefined plugin fails referential-integrity validation",
    );
}
