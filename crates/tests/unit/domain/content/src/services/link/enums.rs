//! Tests for LinkType, DestinationType enums and destination type detection

use systemprompt_content::models::CampaignLink;
use systemprompt_identifiers::LinkId;

// ============================================================================
// Destination Type Detection Tests (via build_trackable_url behavior)
// ============================================================================

#[test]
fn test_destination_type_internal_slash_prefix() {
    let link = CampaignLink {
        id: LinkId::new("internal-link"),
        short_code: "int123".to_string(),
        target_url: "/blog/article".to_string(),
        link_type: "redirect".to_string(),
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: None,
        link_text: None,
        link_position: None,
        destination_type: Some("internal".to_string()),
        click_count: None,
        unique_click_count: None,
        conversion_count: None,
        is_active: Some(true),
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    assert_eq!(link.destination_type, Some("internal".to_string()));
}

#[test]
fn test_destination_type_internal_localhost() {
    let link = CampaignLink {
        id: LinkId::new("localhost-link"),
        short_code: "loc123".to_string(),
        target_url: "http://localhost:3000/page".to_string(),
        link_type: "redirect".to_string(),
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: None,
        link_text: None,
        link_position: None,
        destination_type: Some("internal".to_string()),
        click_count: None,
        unique_click_count: None,
        conversion_count: None,
        is_active: Some(true),
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    assert_eq!(link.destination_type, Some("internal".to_string()));
}

#[test]
fn test_destination_type_internal_systemprompt_domain() {
    let link = CampaignLink {
        id: LinkId::new("sp-link"),
        short_code: "sp123".to_string(),
        target_url: "https://systemprompt.io/agents".to_string(),
        link_type: "redirect".to_string(),
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: None,
        link_text: None,
        link_position: None,
        destination_type: Some("internal".to_string()),
        click_count: None,
        unique_click_count: None,
        conversion_count: None,
        is_active: Some(true),
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    assert_eq!(link.destination_type, Some("internal".to_string()));
}

#[test]
fn test_destination_type_internal_tyingshoelaces_domain() {
    let link = CampaignLink {
        id: LinkId::new("ts-link"),
        short_code: "ts123".to_string(),
        target_url: "https://tyingshoelaces.com/blog".to_string(),
        link_type: "redirect".to_string(),
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: None,
        link_text: None,
        link_position: None,
        destination_type: Some("internal".to_string()),
        click_count: None,
        unique_click_count: None,
        conversion_count: None,
        is_active: Some(true),
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    assert_eq!(link.destination_type, Some("internal".to_string()));
}

#[test]
fn test_destination_type_external_generic() {
    let link = CampaignLink {
        id: LinkId::new("ext-link"),
        short_code: "ext123".to_string(),
        target_url: "https://example.com/page".to_string(),
        link_type: "redirect".to_string(),
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: None,
        link_text: None,
        link_position: None,
        destination_type: Some("external".to_string()),
        click_count: None,
        unique_click_count: None,
        conversion_count: None,
        is_active: Some(true),
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    assert_eq!(link.destination_type, Some("external".to_string()));
}

#[test]
fn test_destination_type_external_github() {
    let link = CampaignLink {
        id: LinkId::new("gh-link"),
        short_code: "gh123".to_string(),
        target_url: "https://github.com/anthropics/claude-code".to_string(),
        link_type: "redirect".to_string(),
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: None,
        link_text: None,
        link_position: None,
        destination_type: Some("external".to_string()),
        click_count: None,
        unique_click_count: None,
        conversion_count: None,
        is_active: Some(true),
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    assert_eq!(link.destination_type, Some("external".to_string()));
}

// ============================================================================
// LinkType Enum Tests
// ============================================================================

#[test]
fn test_link_type_redirect_as_str() {
    use systemprompt_content::models::LinkType;
    assert_eq!(LinkType::Redirect.as_str(), "redirect");
}

#[test]
fn test_link_type_utm_as_str() {
    use systemprompt_content::models::LinkType;
    assert_eq!(LinkType::Utm.as_str(), "utm");
}

#[test]
fn test_link_type_both_as_str() {
    use systemprompt_content::models::LinkType;
    assert_eq!(LinkType::Both.as_str(), "both");
}

#[test]
fn test_link_type_display_redirect() {
    use systemprompt_content::models::LinkType;
    assert_eq!(format!("{}", LinkType::Redirect), "redirect");
}

#[test]
fn test_link_type_display_utm() {
    use systemprompt_content::models::LinkType;
    assert_eq!(format!("{}", LinkType::Utm), "utm");
}

#[test]
fn test_link_type_display_both() {
    use systemprompt_content::models::LinkType;
    assert_eq!(format!("{}", LinkType::Both), "both");
}

#[test]
fn test_link_type_serialize() {
    use systemprompt_content::models::LinkType;

    assert_eq!(serde_json::to_string(&LinkType::Redirect).unwrap(), "\"Redirect\"");
    assert_eq!(serde_json::to_string(&LinkType::Utm).unwrap(), "\"Utm\"");
    assert_eq!(serde_json::to_string(&LinkType::Both).unwrap(), "\"Both\"");
}

// ============================================================================
// DestinationType Enum Tests
// ============================================================================

#[test]
fn test_destination_type_internal_as_str() {
    use systemprompt_content::models::DestinationType;
    assert_eq!(DestinationType::Internal.as_str(), "internal");
}

#[test]
fn test_destination_type_external_as_str() {
    use systemprompt_content::models::DestinationType;
    assert_eq!(DestinationType::External.as_str(), "external");
}

#[test]
fn test_destination_type_display_internal() {
    use systemprompt_content::models::DestinationType;
    assert_eq!(format!("{}", DestinationType::Internal), "internal");
}

#[test]
fn test_destination_type_display_external() {
    use systemprompt_content::models::DestinationType;
    assert_eq!(format!("{}", DestinationType::External), "external");
}

#[test]
fn test_destination_type_serialize() {
    use systemprompt_content::models::DestinationType;

    assert_eq!(serde_json::to_string(&DestinationType::Internal).unwrap(), "\"Internal\"");
    assert_eq!(serde_json::to_string(&DestinationType::External).unwrap(), "\"External\"");
}

// ============================================================================
// CampaignLink.get_full_url Tests
// ============================================================================

#[test]
fn test_campaign_link_get_full_url_no_utm() {
    let link = CampaignLink {
        id: LinkId::new("link"),
        short_code: "code".to_string(),
        target_url: "https://example.com/page".to_string(),
        link_type: "redirect".to_string(),
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: None,
        link_text: None,
        link_position: None,
        destination_type: None,
        click_count: None,
        unique_click_count: None,
        conversion_count: None,
        is_active: None,
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    let result = link.get_full_url();
    assert_eq!(result, "https://example.com/page");
}

#[test]
fn test_campaign_link_get_full_url_with_utm() {
    let link = CampaignLink {
        id: LinkId::new("link"),
        short_code: "code".to_string(),
        target_url: "https://example.com/page".to_string(),
        link_type: "both".to_string(),
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: Some(r#"{"source":"google","medium":"cpc"}"#.to_string()),
        link_text: None,
        link_position: None,
        destination_type: None,
        click_count: None,
        unique_click_count: None,
        conversion_count: None,
        is_active: None,
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    let result = link.get_full_url();
    assert!(result.contains("utm_source=google"));
    assert!(result.contains("utm_medium=cpc"));
    assert!(result.starts_with("https://example.com/page?"));
}

#[test]
fn test_campaign_link_get_full_url_with_existing_query() {
    let link = CampaignLink {
        id: LinkId::new("link"),
        short_code: "code".to_string(),
        target_url: "https://example.com/page?existing=param".to_string(),
        link_type: "both".to_string(),
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: Some(r#"{"source":"twitter"}"#.to_string()),
        link_text: None,
        link_position: None,
        destination_type: None,
        click_count: None,
        unique_click_count: None,
        conversion_count: None,
        is_active: None,
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    let result = link.get_full_url();
    assert!(result.contains("existing=param"));
    assert!(result.contains("utm_source=twitter"));
    assert!(result.contains("&utm_source"));
}

#[test]
fn test_campaign_link_get_full_url_invalid_utm_json() {
    let link = CampaignLink {
        id: LinkId::new("link"),
        short_code: "code".to_string(),
        target_url: "https://example.com/page".to_string(),
        link_type: "both".to_string(),
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: Some("invalid json".to_string()),
        link_text: None,
        link_position: None,
        destination_type: None,
        click_count: None,
        unique_click_count: None,
        conversion_count: None,
        is_active: None,
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    let result = link.get_full_url();
    assert_eq!(result, "https://example.com/page");
}
