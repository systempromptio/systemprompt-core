use systemprompt_bridge::gui::server_marketplace::source::{
    MarketplaceCategory, MarketplaceSource, MarketplaceSourceCtx, MarketplaceSourceRegistration,
};
use systemprompt_bridge::gui::server_marketplace::{
    MarketplaceItem, build_listing, listing_to_value,
};
use systemprompt_bridge::register_marketplace_source;

struct TestSkillsSource;

impl MarketplaceSource for TestSkillsSource {
    fn category(&self) -> MarketplaceCategory {
        MarketplaceCategory::Skills
    }
    fn items(&self, _ctx: &MarketplaceSourceCtx<'_>) -> Vec<MarketplaceItem> {
        vec![MarketplaceItem::new(
            "test-skill",
            "Test Skill",
            None,
            String::new(),
            "test",
        )]
    }
}

register_marketplace_source!(TestSkillsSource);

#[test]
fn externally_registered_source_is_iterated() {
    let ctx = MarketplaceSourceCtx {
        plugins_root: None,
        mcp_auth: &[],
    };
    let found = inventory::iter::<MarketplaceSourceRegistration>().any(|reg| {
        matches!(reg.source.category(), MarketplaceCategory::Skills)
            && reg
                .source
                .items(&ctx)
                .iter()
                .any(|i| item_id(i) == Some("test-skill".to_owned()))
    });
    assert!(
        found,
        "marketplace source registered via macro not iterated"
    );
}

struct ShadowHigh;
struct ShadowLow;

impl MarketplaceSource for ShadowHigh {
    fn category(&self) -> MarketplaceCategory {
        MarketplaceCategory::Skills
    }
    fn items(&self, _ctx: &MarketplaceSourceCtx<'_>) -> Vec<MarketplaceItem> {
        vec![MarketplaceItem::new(
            "dup-skill",
            "High",
            None,
            String::new(),
            "high",
        )]
    }
}

impl MarketplaceSource for ShadowLow {
    fn category(&self) -> MarketplaceCategory {
        MarketplaceCategory::Skills
    }
    fn items(&self, _ctx: &MarketplaceSourceCtx<'_>) -> Vec<MarketplaceItem> {
        vec![MarketplaceItem::new(
            "dup-skill",
            "Low",
            None,
            String::new(),
            "low",
        )]
    }
}

register_marketplace_source!(ShadowHigh, priority = 50);
register_marketplace_source!(ShadowLow, priority = 5);

#[test]
fn higher_priority_source_shadows_same_id_item() {
    let listing = build_listing(&[]);
    let value = listing_to_value(&listing).expect("serialize listing");
    let skills = value["skills"].as_array().expect("skills array");

    let dups: Vec<&serde_json::Value> = skills
        .iter()
        .filter(|item| item.get("id").and_then(|v| v.as_str()) == Some("dup-skill"))
        .collect();

    assert_eq!(dups.len(), 1, "same-id items must dedup to one");
    assert_eq!(
        dups[0].get("name").and_then(|v| v.as_str()),
        Some("High"),
        "the higher-priority source's item must win the shadow"
    );
}

fn item_id(item: &MarketplaceItem) -> Option<String> {
    serde_json::to_value(item)
        .ok()
        .and_then(|v| v.get("id").and_then(|id| id.as_str()).map(str::to_owned))
}
