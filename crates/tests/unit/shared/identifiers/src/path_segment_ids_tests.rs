use systemprompt_identifiers::{MarketplaceId, PluginId, SkillId};

const REFUSED: [&str; 9] = [
    "",
    "   ",
    ".",
    "..",
    "../escape",
    "a/b",
    "a\\b",
    "/etc",
    "nul\0byte",
];

#[test]
fn services_tree_ids_refuse_values_that_are_not_one_path_segment() {
    for value in REFUSED {
        assert!(
            SkillId::try_new(value).is_err(),
            "SkillId accepted {value:?}"
        );
        assert!(
            PluginId::try_new(value).is_err(),
            "PluginId accepted {value:?}"
        );
        assert!(
            MarketplaceId::try_new(value).is_err(),
            "MarketplaceId accepted {value:?}"
        );
    }
}

#[test]
fn services_tree_ids_accept_plain_slugs() {
    for value in ["writer", "plugin_core", "mkt-default", ".hidden", "v1.2"] {
        assert_eq!(SkillId::try_new(value).unwrap().as_str(), value);
        assert_eq!(PluginId::try_new(value).unwrap().as_str(), value);
        assert_eq!(MarketplaceId::try_new(value).unwrap().as_str(), value);
    }
}

#[test]
fn services_tree_ids_refuse_traversal_when_deserialized() {
    assert!(serde_json::from_str::<SkillId>("\"../x\"").is_err());
    assert!(serde_json::from_str::<PluginId>("\"a/b\"").is_err());
    assert!(serde_json::from_str::<MarketplaceId>("\"..\"").is_err());
    assert!("../x".parse::<PluginId>().is_err());
}
