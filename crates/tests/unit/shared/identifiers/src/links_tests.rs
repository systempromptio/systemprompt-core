use systemprompt_identifiers::{LinkId, CampaignId, LinkClickId, DbValue, ToDbValue};

#[test]
fn link_id_generate_uuid_format() {
    let id = LinkId::generate();
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn link_id_generate_unique() {
    let id1 = LinkId::generate();
    let id2 = LinkId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn link_id_serde_transparent() {
    let id = LinkId::new("link-1");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"link-1\"");
    let deserialized: LinkId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn link_click_id_generate_uuid_format() {
    let id = LinkClickId::generate();
    assert_eq!(id.as_str().len(), 36);
}

#[test]
fn link_click_id_generate_unique() {
    let id1 = LinkClickId::generate();
    let id2 = LinkClickId::generate();
    assert_ne!(id1, id2);
}

#[test]
fn campaign_id_serde_transparent() {
    let id = CampaignId::new("camp-1");
    let json = serde_json::to_string(&id).unwrap();
    assert_eq!(json, "\"camp-1\"");
    let deserialized: CampaignId = serde_json::from_str(&json).unwrap();
    assert_eq!(deserialized, id);
}

#[test]
fn all_link_ids_to_db_value() {
    assert!(matches!(LinkId::new("a").to_db_value(), DbValue::String(ref s) if s == "a"));
    assert!(matches!(CampaignId::new("b").to_db_value(), DbValue::String(ref s) if s == "b"));
    assert!(matches!(LinkClickId::new("c").to_db_value(), DbValue::String(ref s) if s == "c"));
}

#[test]
fn all_link_ids_ref_to_db_value() {
    let link = LinkId::new("a");
    assert!(matches!((&link).to_db_value(), DbValue::String(ref s) if s == "a"));
    let camp = CampaignId::new("b");
    assert!(matches!((&camp).to_db_value(), DbValue::String(ref s) if s == "b"));
    let click = LinkClickId::new("c");
    assert!(matches!((&click).to_db_value(), DbValue::String(ref s) if s == "c"));
}

#[test]
fn all_link_ids_into_string() {
    let s: String = LinkId::new("l").into();
    assert_eq!(s, "l");
    let s: String = CampaignId::new("c").into();
    assert_eq!(s, "c");
    let s: String = LinkClickId::new("k").into();
    assert_eq!(s, "k");
}

#[test]
fn all_link_ids_from_str_and_string_equal() {
    let a: LinkId = "x".into();
    let b: LinkId = String::from("x").into();
    assert_eq!(a, b);
    let a: CampaignId = "y".into();
    let b: CampaignId = String::from("y").into();
    assert_eq!(a, b);
}
