use systemprompt_bridge::gateway::catalog::{CatalogFreshness, InferenceCatalog};
use systemprompt_bridge::gateway::model_view::{catalog_models, desktop_models};
use systemprompt_bridge::gateway::types::ProviderHealth;

const GATEWAY: &str = "https://gateway.example.com";

fn providers() -> Vec<ProviderHealth> {
    serde_json::from_value(serde_json::json!([
        { "name": "vertex-claude", "surface": "anthropic", "configured": true,
          "models": ["claude-opus-5-5", "claude-sonnet-5"] },
        { "name": "vertex-gemini", "surface": "gemini", "configured": true,
          "models": ["gemini-3-pro", "claude-opus-5-5"] },
        { "name": "glm", "surface": "openai", "configured": false,
          "models": [], "config_issue": "secret ZAI_API_KEY is absent" },
        { "name": "minimax", "surface": "backend", "configured": true,
          "models": ["MiniMax-M2"] }
    ]))
    .expect("provider fixture parses")
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
    catalog.apply(GATEWAY, 160, Err("timed out".to_owned()));
    catalog.apply(GATEWAY, 220, Err("timed out again".to_owned()));

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
    catalog.apply(GATEWAY, 100, Err("unreachable".to_owned()));
    assert_eq!(catalog.freshness, CatalogFreshness::Unknown);
    assert!(catalog.providers.is_empty());
}

#[test]
fn another_gateway_never_inherits_the_held_providers() {
    let mut catalog = fresh();
    catalog.apply(
        "https://other.example.com",
        160,
        Err("unreachable".to_owned()),
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

#[test]
fn desktop_models_keep_claude_ids_once() {
    let models: Vec<String> = [
        "claude-opus-5-5",
        "gemini-3-pro",
        "claude-fable-5-1",
        "claude-opus-5-5",
        "glm-5",
    ]
    .iter()
    .map(|s| (*s).to_owned())
    .collect();
    assert_eq!(desktop_models(&models), ["claude-opus-5-5"]);
}
