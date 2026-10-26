use systemprompt_bridge::gateway::catalog::{
    CatalogFetchError, CatalogFreshness, InferenceCatalog,
};
use systemprompt_bridge::gateway::model_view::catalog_models;
use systemprompt_bridge::gateway::types::ProviderHealth;

const GATEWAY: &str = "https://gateway.example.com";

fn providers() -> Vec<ProviderHealth> {
    serde_json::from_value(serde_json::json!([
        { "name": "vertex-claude", "credential": "vertex", "clients": ["claude"], "configured": true,
          "models": ["claude-opus-5-5", "claude-sonnet-5"] },
        { "name": "vertex-gemini", "credential": "vertex", "clients": ["gemini"], "configured": true,
          "models": ["gemini-3-pro", "claude-opus-5-5"] },
        { "name": "glm", "credential": "zai", "clients": ["openai"], "configured": false,
          "models": [], "config_issue": "secret ZAI_API_KEY is absent" },
        { "name": "minimax", "credential": "minimax", "clients": [], "configured": true,
          "models": ["MiniMax-M2"] }
    ]))
    .expect("provider fixture parses")
}

fn unreachable(reason: &str) -> CatalogFetchError {
    CatalogFetchError::Unreachable {
        reason: reason.to_owned(),
    }
}

fn fresh() -> InferenceCatalog {
    let mut catalog = InferenceCatalog::default();
    catalog.apply(GATEWAY, 100, Ok(providers()));
    catalog
}

#[test]
fn a_good_fetch_replaces_the_catalog_and_marks_it_fresh() {
    let catalog = fresh();
    assert_eq!(catalog.freshness, CatalogFreshness::Fresh);
    assert_eq!(catalog.fetched_at_unix, Some(100));
    assert_eq!(catalog.providers.len(), 4);
}

#[test]
fn a_failed_fetch_keeps_the_last_good_providers_and_marks_them_stale() {
    let mut catalog = fresh();
    catalog.apply(GATEWAY, 160, Err(unreachable("timed out")));
    catalog.apply(GATEWAY, 220, Err(unreachable("timed out again")));

    assert_eq!(
        catalog.providers.len(),
        4,
        "a slow probe must not empty the card"
    );
    assert_eq!(catalog.fetched_at_unix, Some(100));
    assert_eq!(
        catalog.freshness,
        CatalogFreshness::Stale {
            since: 160,
            error: "timed out again".to_owned(),
        },
        "a run of failures keeps the age of the first"
    );

    catalog.apply(GATEWAY, 280, Ok(providers()));
    assert_eq!(catalog.freshness, CatalogFreshness::Fresh);
}

#[test]
fn a_failure_with_nothing_held_stays_unknown() {
    let mut catalog = InferenceCatalog::default();
    catalog.apply(GATEWAY, 100, Err(unreachable("unreachable")));
    assert_eq!(catalog.freshness, CatalogFreshness::Unknown);
    assert!(catalog.providers.is_empty());
}

#[test]
fn another_gateway_never_inherits_the_held_providers() {
    let mut catalog = fresh();
    catalog.apply(
        "https://other.example.com",
        160,
        Err(unreachable("unreachable")),
    );
    assert!(catalog.providers.is_empty());
    assert_eq!(catalog.freshness, CatalogFreshness::Unknown);
}

#[test]
fn catalog_groups_advertised_providers_with_distinct_models() {
    let groups = catalog_models(&providers());
    let names: Vec<&str> = groups.iter().map(|g| g.name.as_str()).collect();
    assert_eq!(names, ["vertex-claude", "vertex-gemini", "glm"]);
    assert_eq!(groups[1].models, ["gemini-3-pro"]);
    assert!(!groups[2].configured);
    assert_eq!(
        groups[2].config_issue.as_deref(),
        Some("secret ZAI_API_KEY is absent")
    );
}
