//! `env.SLASH_COMMAND_TOOL_CHAR_BUDGET`: the largest marketplace budget is
//! written, a larger value the user set is kept, and only the value the bridge
//! wrote is taken back out.

use serde_json::{Map, Value, json};
use systemprompt_bridge::gateway::manifest::{
    ManifestClaudeCode, ManifestMarketplace, SignedManifest, SignedManifestBuilder,
};
use systemprompt_bridge::gateway::manifest_version::ManifestVersion;
use systemprompt_bridge::install::mdm::claude_code_settings::skill_budget::{
    BUDGET_ENV, budget_for, splice_budget,
};

fn root(value: Value) -> Map<String, Value> {
    value.as_object().cloned().expect("object")
}

fn marketplace(id: &str, budget: Option<u32>) -> ManifestMarketplace {
    ManifestMarketplace {
        id: systemprompt_identifiers::MarketplaceId::new(id),
        name: id.to_owned(),
        plugin_ids: Vec::new(),
        allow_cross_marketplace_dependencies_on: Vec::new(),
        external_marketplaces: Vec::new(),
        external_plugins: vec![],
        claude_code: budget.map(|chars| ManifestClaudeCode {
            skill_listing_budget_chars: Some(chars),
        }),
    }
}

fn manifest(marketplaces: Vec<ManifestMarketplace>) -> SignedManifest {
    let at = chrono::DateTime::parse_from_rfc3339("2026-09-30T00:00:00Z")
        .expect("rfc3339")
        .with_timezone(&chrono::Utc);
    SignedManifestBuilder::new(
        ManifestVersion::try_new("2026-09-30T00:00:00Z-00000000").unwrap(),
        at,
        at,
        systemprompt_identifiers::UserId::new("00000000-0000-4000-8000-00000000beef"),
    )
    .with_marketplaces(marketplaces)
    .build()
}

#[test]
fn the_largest_budget_across_marketplaces_wins() {
    let m = manifest(vec![
        marketplace("small", Some(20_000)),
        marketplace("none", None),
        marketplace("large", Some(50_000)),
    ]);

    assert_eq!(budget_for(&m), Some(50_000));
    assert_eq!(budget_for(&manifest(vec![marketplace("none", None)])), None);
}

#[test]
fn writes_the_budget_as_a_string_beside_the_users_env() {
    let mut settings = root(json!({"env": {"MY_OWN": "keep"}, "apiKeyHelper": "h"}));

    assert!(splice_budget(&mut settings, None, Some(50_000)));

    assert_eq!(settings["env"][BUDGET_ENV], json!("50000"));
    assert_eq!(settings["env"]["MY_OWN"], json!("keep"));
}

#[test]
fn creates_the_env_object_when_absent() {
    let mut settings = root(json!({}));

    assert!(splice_budget(&mut settings, None, Some(50_000)));

    assert_eq!(settings, root(json!({"env": {BUDGET_ENV: "50000"}})));
}

#[test]
fn a_larger_user_value_is_kept() {
    let mut settings = root(json!({"env": {BUDGET_ENV: "80000"}}));

    assert!(!splice_budget(&mut settings, None, Some(50_000)));

    assert_eq!(settings["env"][BUDGET_ENV], json!("80000"));
}

#[test]
fn a_smaller_user_value_is_raised() {
    let mut settings = root(json!({"env": {BUDGET_ENV: "8000"}}));

    assert!(splice_budget(&mut settings, None, Some(50_000)));

    assert_eq!(settings["env"][BUDGET_ENV], json!("50000"));
}

#[test]
fn the_bridges_own_value_follows_the_marketplace_down() {
    let mut settings = root(json!({"env": {BUDGET_ENV: "60000"}}));

    assert!(splice_budget(&mut settings, Some("60000"), Some(50_000)));

    assert_eq!(settings["env"][BUDGET_ENV], json!("50000"));
}

#[test]
fn removes_only_the_value_the_bridge_wrote() {
    let mut ours = root(json!({"env": {BUDGET_ENV: "50000", "MY_OWN": "keep"}}));
    assert!(splice_budget(&mut ours, Some("50000"), None));
    assert_eq!(ours, root(json!({"env": {"MY_OWN": "keep"}})));

    let mut only_ours = root(json!({"env": {BUDGET_ENV: "50000"}, "model": "m"}));
    assert!(splice_budget(&mut only_ours, Some("50000"), None));
    assert_eq!(only_ours, root(json!({"model": "m"})));

    let mut users = root(json!({"env": {BUDGET_ENV: "70000"}}));
    assert!(!splice_budget(&mut users, Some("50000"), None));
    assert_eq!(users["env"][BUDGET_ENV], json!("70000"));

    let mut never_written = root(json!({"env": {BUDGET_ENV: "70000"}}));
    assert!(!splice_budget(&mut never_written, None, None));
    assert_eq!(never_written["env"][BUDGET_ENV], json!("70000"));
}

#[test]
fn a_non_object_env_is_left_alone() {
    let mut settings = root(json!({"env": "not-an-object"}));

    assert!(!splice_budget(&mut settings, None, Some(50_000)));

    assert_eq!(settings["env"], json!("not-an-object"));
}
