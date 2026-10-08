//! The `claude_code` block on a manifest marketplace: round trip, omission
//! when absent, and tolerance of keys a newer gateway adds.

use serde_json::json;
use systemprompt_bridge::gateway::manifest::{ManifestClaudeCode, ManifestMarketplace};

fn marketplace(claude_code: Option<ManifestClaudeCode>) -> ManifestMarketplace {
    ManifestMarketplace {
        id: systemprompt_identifiers::MarketplaceId::new("acme"),
        name: "Acme".to_owned(),
        plugin_ids: Vec::new(),
        allow_cross_marketplace_dependencies_on: Vec::new(),
        external_marketplaces: Vec::new(),
        external_plugins: vec![],
        claude_code,
    }
}

#[test]
fn a_skill_listing_budget_round_trips() {
    let original = marketplace(Some(ManifestClaudeCode {
        skill_listing_budget_chars: Some(50_000),
    }));
    let value = serde_json::to_value(&original).expect("serializes");

    assert_eq!(
        value["claude_code"],
        json!({"skill_listing_budget_chars": 50_000})
    );
    let back: ManifestMarketplace = serde_json::from_value(value).expect("parses");
    assert_eq!(back, original);
}

#[test]
fn an_absent_block_is_not_serialized_and_parses_as_none() {
    let value = serde_json::to_value(marketplace(None)).expect("serializes");

    assert!(value.get("claude_code").is_none(), "{value}");
    let back: ManifestMarketplace = serde_json::from_value(value).expect("parses");
    assert_eq!(back.claude_code, None);
}

#[test]
fn unknown_keys_from_a_newer_gateway_are_ignored() {
    let value = json!({
        "id": "acme",
        "name": "Acme",
        "plugin_ids": [],
        "claude_code": {"skill_listing_budget_chars": 60_000, "future_setting": true}
    });

    let parsed: ManifestMarketplace = serde_json::from_value(value).expect("tolerant parse");

    assert_eq!(
        parsed.claude_code,
        Some(ManifestClaudeCode {
            skill_listing_budget_chars: Some(60_000)
        })
    );
}
