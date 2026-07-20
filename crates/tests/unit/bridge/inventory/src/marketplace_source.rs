use systemprompt_bridge::gui::server_marketplace::MarketplaceItem;
use systemprompt_bridge::gui::server_marketplace::source::{
    MarketplaceCategory, MarketplaceSource, MarketplaceSourceCtx, MarketplaceSourceRegistration,
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
        matches!(reg.0.category(), MarketplaceCategory::Skills)
            && reg.0.items(&ctx).iter().any(|i| {
                serde_json::to_value(i)
                    .ok()
                    .and_then(|v| v.get("id").and_then(|id| id.as_str()).map(str::to_owned))
                    .as_deref()
                    == Some("test-skill")
            })
    });
    assert!(
        found,
        "marketplace source registered via macro not iterated"
    );
}
