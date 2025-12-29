//! Unit tests for link-related identifier types.

use std::collections::HashSet;
use systemprompt_identifiers::{LinkId, CampaignId, LinkClickId, ToDbValue, DbValue};

// ============================================================================
// LinkId Tests
// ============================================================================

#[test]
fn test_link_id_new() {
    let id = LinkId::new("link-123");
    assert_eq!(id.as_str(), "link-123");
}

#[test]
fn test_link_id_generate() {
    let id = LinkId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_link_id_generate_unique() {
    let id1 = LinkId::generate();
    let id2 = LinkId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_link_id_display() {
    let id = LinkId::new("display-link");
    assert_eq!(format!("{}", id), "display-link");
}

#[test]
fn test_link_id_from_string() {
    let id: LinkId = String::from("from-string-link").into();
    assert_eq!(id.as_str(), "from-string-link");
}

#[test]
fn test_link_id_from_str() {
    let id: LinkId = "from-str-link".into();
    assert_eq!(id.as_str(), "from-str-link");
}

#[test]
fn test_link_id_as_ref() {
    let id = LinkId::new("as-ref-link");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-link");
}

#[test]
fn test_link_id_clone_and_eq() {
    let id1 = LinkId::new("clone-link");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_link_id_hash() {
    let id1 = LinkId::new("hash-link");
    let id2 = LinkId::new("hash-link");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_link_id_serialize_json() {
    let id = LinkId::new("serialize-link");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-link\"");
}

#[test]
fn test_link_id_deserialize_json() {
    let id: LinkId = serde_json::from_str("\"deserialize-link\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-link");
}

#[test]
fn test_link_id_to_db_value() {
    let id = LinkId::new("db-value-link");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-link"));
}

#[test]
fn test_link_id_ref_to_db_value() {
    let id = LinkId::new("db-value-ref-link");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-link"));
}

// ============================================================================
// CampaignId Tests
// ============================================================================

#[test]
fn test_campaign_id_new() {
    let id = CampaignId::new("campaign-123");
    assert_eq!(id.as_str(), "campaign-123");
}

#[test]
fn test_campaign_id_display() {
    let id = CampaignId::new("display-campaign");
    assert_eq!(format!("{}", id), "display-campaign");
}

#[test]
fn test_campaign_id_from_string() {
    let id: CampaignId = String::from("from-string-campaign").into();
    assert_eq!(id.as_str(), "from-string-campaign");
}

#[test]
fn test_campaign_id_from_str() {
    let id: CampaignId = "from-str-campaign".into();
    assert_eq!(id.as_str(), "from-str-campaign");
}

#[test]
fn test_campaign_id_as_ref() {
    let id = CampaignId::new("as-ref-campaign");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-campaign");
}

#[test]
fn test_campaign_id_clone_and_eq() {
    let id1 = CampaignId::new("clone-campaign");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_campaign_id_hash() {
    let id1 = CampaignId::new("hash-campaign");
    let id2 = CampaignId::new("hash-campaign");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_campaign_id_serialize_json() {
    let id = CampaignId::new("serialize-campaign");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-campaign\"");
}

#[test]
fn test_campaign_id_deserialize_json() {
    let id: CampaignId = serde_json::from_str("\"deserialize-campaign\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-campaign");
}

#[test]
fn test_campaign_id_to_db_value() {
    let id = CampaignId::new("db-value-campaign");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-campaign"));
}

#[test]
fn test_campaign_id_ref_to_db_value() {
    let id = CampaignId::new("db-value-ref-campaign");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-campaign"));
}

// ============================================================================
// LinkClickId Tests
// ============================================================================

#[test]
fn test_link_click_id_new() {
    let id = LinkClickId::new("click-123");
    assert_eq!(id.as_str(), "click-123");
}

#[test]
fn test_link_click_id_generate() {
    let id = LinkClickId::generate();
    assert!(!id.as_str().is_empty());
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn test_link_click_id_generate_unique() {
    let id1 = LinkClickId::generate();
    let id2 = LinkClickId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn test_link_click_id_display() {
    let id = LinkClickId::new("display-click");
    assert_eq!(format!("{}", id), "display-click");
}

#[test]
fn test_link_click_id_from_string() {
    let id: LinkClickId = String::from("from-string-click").into();
    assert_eq!(id.as_str(), "from-string-click");
}

#[test]
fn test_link_click_id_from_str() {
    let id: LinkClickId = "from-str-click".into();
    assert_eq!(id.as_str(), "from-str-click");
}

#[test]
fn test_link_click_id_as_ref() {
    let id = LinkClickId::new("as-ref-click");
    let s: &str = id.as_ref();
    assert_eq!(s, "as-ref-click");
}

#[test]
fn test_link_click_id_clone_and_eq() {
    let id1 = LinkClickId::new("clone-click");
    let id2 = id1.clone();
    assert_eq!(id1, id2);
}

#[test]
fn test_link_click_id_hash() {
    let id1 = LinkClickId::new("hash-click");
    let id2 = LinkClickId::new("hash-click");

    let mut set = HashSet::new();
    set.insert(id1.clone());
    assert!(set.contains(&id2));
}

#[test]
fn test_link_click_id_serialize_json() {
    let id = LinkClickId::new("serialize-click");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"serialize-click\"");
}

#[test]
fn test_link_click_id_deserialize_json() {
    let id: LinkClickId = serde_json::from_str("\"deserialize-click\"").unwrap();
    assert_eq!(id.as_str(), "deserialize-click");
}

#[test]
fn test_link_click_id_to_db_value() {
    let id = LinkClickId::new("db-value-click");
    let db_value = id.to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-click"));
}

#[test]
fn test_link_click_id_ref_to_db_value() {
    let id = LinkClickId::new("db-value-ref-click");
    let db_value = (&id).to_db_value();
    assert!(matches!(db_value, DbValue::String(s) if s == "db-value-ref-click"));
}
