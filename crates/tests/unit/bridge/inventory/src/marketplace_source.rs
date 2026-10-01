use systemprompt_bridge::gui::server_marketplace::source::{
    MarketplaceCategory, MarketplaceSource, MarketplaceSourceCtx, MarketplaceSourceRegistration,
};
use systemprompt_bridge::gui::server_marketplace::{MarketplaceItem, build_listing};
use systemprompt_bridge::proxy::LoopbackEndpoint;
use systemprompt_bridge::{mcp_registry, register_marketplace_source};

struct TestSkillsSource;

impl MarketplaceSource for TestSkillsSource {
    fn category(&self) -> MarketplaceCategory {
        MarketplaceCategory::Skills
    }
    fn items(&self, _ctx: &MarketplaceSourceCtx<'_>) -> Vec<MarketplaceItem> {
        vec![
            MarketplaceItem::builder("test-skill", String::new())
                .name("Test Skill")
                .build(),
        ]
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
        vec![
            MarketplaceItem::builder("dup-skill", String::new())
                .name("High")
                .build(),
        ]
    }
}

impl MarketplaceSource for ShadowLow {
    fn category(&self) -> MarketplaceCategory {
        MarketplaceCategory::Skills
    }
    fn items(&self, _ctx: &MarketplaceSourceCtx<'_>) -> Vec<MarketplaceItem> {
        vec![
            MarketplaceItem::builder("dup-skill", String::new())
                .name("Low")
                .build(),
        ]
    }
}

register_marketplace_source!(ShadowHigh, priority = 50);
register_marketplace_source!(ShadowLow, priority = 5);

#[test]
fn higher_priority_source_shadows_same_id_item() {
    let sandbox = tempfile::TempDir::new().expect("sandbox tempdir");
    let root = sandbox.path();
    let listing = temp_env::with_vars(
        [
            ("HOME", Some(root.display().to_string())),
            (
                "SP_BRIDGE_ORG_PLUGINS_SYSTEM",
                Some(root.join("org-plugins").display().to_string()),
            ),
            (
                "XDG_CONFIG_HOME",
                Some(root.join("config").display().to_string()),
            ),
            (
                "XDG_DATA_HOME",
                Some(root.join("data").display().to_string()),
            ),
            (
                "XDG_STATE_HOME",
                Some(root.join("state").display().to_string()),
            ),
            (
                "XDG_CACHE_HOME",
                Some(root.join("cache").display().to_string()),
            ),
        ],
        || {
            let loopback = LoopbackEndpoint::new(9999, None);
            let registry = mcp_registry::snapshot(&mcp_registry::empty_slot());
            build_listing(&loopback, &registry, &[]).expect("the listing builds")
        },
    );
    let value = serde_json::to_value(&listing).expect("serialize listing");
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
