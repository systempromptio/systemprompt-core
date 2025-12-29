//! Unit tests for link models
//!
//! Tests cover:
//! - LinkType enum (as_str, Display impl)
//! - DestinationType enum (as_str, Display impl)
//! - UtmParams (to_query_string, to_json)
//! - CampaignLink (get_full_url)

use systemprompt_core_content::models::{
    DestinationType, LinkType, UtmParams,
};

// ============================================================================
// LinkType Tests
// ============================================================================

#[test]
fn test_link_type_as_str_redirect() {
    let link_type = LinkType::Redirect;
    assert_eq!(link_type.as_str(), "redirect");
}

#[test]
fn test_link_type_as_str_utm() {
    let link_type = LinkType::Utm;
    assert_eq!(link_type.as_str(), "utm");
}

#[test]
fn test_link_type_as_str_both() {
    let link_type = LinkType::Both;
    assert_eq!(link_type.as_str(), "both");
}

#[test]
fn test_link_type_display() {
    assert_eq!(format!("{}", LinkType::Redirect), "redirect");
    assert_eq!(format!("{}", LinkType::Utm), "utm");
    assert_eq!(format!("{}", LinkType::Both), "both");
}

#[test]
fn test_link_type_serialization() {
    let link_type = LinkType::Both;
    let json = serde_json::to_string(&link_type).unwrap();
    assert_eq!(json, "\"Both\"");
}

#[test]
fn test_link_type_deserialization() {
    let link_type: LinkType = serde_json::from_str("\"Redirect\"").unwrap();
    assert!(matches!(link_type, LinkType::Redirect));
}

#[test]
fn test_link_type_copy() {
    let original = LinkType::Utm;
    let copied = original;
    assert_eq!(original.as_str(), copied.as_str());
}

// ============================================================================
// DestinationType Tests
// ============================================================================

#[test]
fn test_destination_type_as_str_internal() {
    let dest_type = DestinationType::Internal;
    assert_eq!(dest_type.as_str(), "internal");
}

#[test]
fn test_destination_type_as_str_external() {
    let dest_type = DestinationType::External;
    assert_eq!(dest_type.as_str(), "external");
}

#[test]
fn test_destination_type_display() {
    assert_eq!(format!("{}", DestinationType::Internal), "internal");
    assert_eq!(format!("{}", DestinationType::External), "external");
}

#[test]
fn test_destination_type_serialization() {
    let dest_type = DestinationType::External;
    let json = serde_json::to_string(&dest_type).unwrap();
    assert_eq!(json, "\"External\"");
}

#[test]
fn test_destination_type_deserialization() {
    let dest_type: DestinationType = serde_json::from_str("\"Internal\"").unwrap();
    assert!(matches!(dest_type, DestinationType::Internal));
}

#[test]
fn test_destination_type_copy() {
    let original = DestinationType::Internal;
    let copied = original;
    assert_eq!(original.as_str(), copied.as_str());
}

// ============================================================================
// UtmParams Tests
// ============================================================================

#[test]
fn test_utm_params_to_query_string_all_params() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: Some("cpc".to_string()),
        campaign: Some("summer_sale".to_string()),
        term: Some("discount".to_string()),
        content: Some("banner_ad".to_string()),
    };

    let query = params.to_query_string();
    assert!(query.contains("utm_source=google"));
    assert!(query.contains("utm_medium=cpc"));
    assert!(query.contains("utm_campaign=summer_sale"));
    assert!(query.contains("utm_term=discount"));
    assert!(query.contains("utm_content=banner_ad"));
    assert_eq!(query.matches('&').count(), 4);
}

#[test]
fn test_utm_params_to_query_string_partial() {
    let params = UtmParams {
        source: Some("twitter".to_string()),
        medium: Some("social".to_string()),
        campaign: None,
        term: None,
        content: None,
    };

    let query = params.to_query_string();
    assert!(query.contains("utm_source=twitter"));
    assert!(query.contains("utm_medium=social"));
    assert!(!query.contains("utm_campaign"));
    assert!(!query.contains("utm_term"));
    assert!(!query.contains("utm_content"));
    assert_eq!(query.matches('&').count(), 1);
}

#[test]
fn test_utm_params_to_query_string_empty() {
    let params = UtmParams {
        source: None,
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let query = params.to_query_string();
    assert!(query.is_empty());
}

#[test]
fn test_utm_params_to_query_string_single_param() {
    let params = UtmParams {
        source: Some("newsletter".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let query = params.to_query_string();
    assert_eq!(query, "utm_source=newsletter");
    assert!(!query.contains('&'));
}

#[test]
fn test_utm_params_to_json() {
    let params = UtmParams {
        source: Some("email".to_string()),
        medium: Some("newsletter".to_string()),
        campaign: Some("weekly".to_string()),
        term: None,
        content: None,
    };

    let json = params.to_json().unwrap();
    assert!(json.contains("\"source\":\"email\""));
    assert!(json.contains("\"medium\":\"newsletter\""));
    assert!(json.contains("\"campaign\":\"weekly\""));
}

#[test]
fn test_utm_params_serialization() {
    let params = UtmParams {
        source: Some("test".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let json = serde_json::to_string(&params).unwrap();
    let deserialized: UtmParams = serde_json::from_str(&json).unwrap();

    assert_eq!(deserialized.source, params.source);
    assert_eq!(deserialized.medium, params.medium);
}

#[test]
fn test_utm_params_clone() {
    let params = UtmParams {
        source: Some("source".to_string()),
        medium: Some("medium".to_string()),
        campaign: Some("campaign".to_string()),
        term: Some("term".to_string()),
        content: Some("content".to_string()),
    };

    let cloned = params.clone();
    assert_eq!(cloned.source, params.source);
    assert_eq!(cloned.medium, params.medium);
    assert_eq!(cloned.campaign, params.campaign);
    assert_eq!(cloned.term, params.term);
    assert_eq!(cloned.content, params.content);
}

// ============================================================================
// LinkPerformance Tests
// ============================================================================

#[test]
fn test_link_performance_serialization() {
    use systemprompt_core_content::models::LinkPerformance;
    use systemprompt_identifiers::LinkId;

    let performance = LinkPerformance {
        link_id: LinkId::new("test_link"),
        click_count: 100,
        unique_click_count: 75,
        conversion_count: 10,
        conversion_rate: Some(0.1),
    };

    let json = serde_json::to_string(&performance).unwrap();
    assert!(json.contains("\"click_count\":100"));
    assert!(json.contains("\"unique_click_count\":75"));
    assert!(json.contains("\"conversion_count\":10"));
}

// ============================================================================
// CampaignPerformance Tests
// ============================================================================

#[test]
fn test_campaign_performance_serialization() {
    use systemprompt_core_content::models::CampaignPerformance;
    use systemprompt_identifiers::CampaignId;

    let performance = CampaignPerformance {
        campaign_id: CampaignId::new("test_campaign"),
        total_clicks: 500,
        link_count: 10,
        unique_visitors: Some(300),
        conversion_count: Some(50),
    };

    let json = serde_json::to_string(&performance).unwrap();
    assert!(json.contains("\"total_clicks\":500"));
    assert!(json.contains("\"link_count\":10"));
}

// ============================================================================
// ContentJourneyNode Tests
// ============================================================================

#[test]
fn test_content_journey_node_creation() {
    use systemprompt_core_content::models::ContentJourneyNode;
    use systemprompt_identifiers::ContentId;

    let node = ContentJourneyNode {
        source_content_id: ContentId::new("content_123"),
        target_url: "https://example.com/target".to_string(),
        click_count: 42,
    };

    assert_eq!(node.source_content_id.as_str(), "content_123");
    assert_eq!(node.target_url, "https://example.com/target");
    assert_eq!(node.click_count, 42);
}

#[test]
fn test_content_journey_node_serialization() {
    use systemprompt_core_content::models::ContentJourneyNode;
    use systemprompt_identifiers::ContentId;

    let node = ContentJourneyNode {
        source_content_id: ContentId::new("src"),
        target_url: "/path".to_string(),
        click_count: 5,
    };

    let json = serde_json::to_string(&node).unwrap();
    assert!(json.contains("\"target_url\":\"/path\""));
    assert!(json.contains("\"click_count\":5"));
}
