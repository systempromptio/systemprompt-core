//! Tests for build_trackable_url and LinkType behavior

use systemprompt_content::models::CampaignLink;
use systemprompt_content::services::LinkGenerationService;
use systemprompt_identifiers::LinkId;

// ============================================================================
// build_trackable_url Tests
// ============================================================================

#[test]
fn test_build_trackable_url_redirect_type() {
    let link = CampaignLink {
        id: LinkId::new("test-link"),
        short_code: "abc123".to_string(),
        target_url: "https://example.com/target".to_string(),
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
        is_active: Some(true),
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    let base_url = "https://short.example.com";
    let result = LinkGenerationService::build_trackable_url(&link, base_url);
    assert_eq!(result, "https://short.example.com/r/abc123");
}

#[test]
fn test_build_trackable_url_both_type() {
    let link = CampaignLink {
        id: LinkId::new("test-link"),
        short_code: "xyz789".to_string(),
        target_url: "https://example.com/target".to_string(),
        link_type: "both".to_string(),
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
        is_active: Some(true),
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    let base_url = "https://app.example.com";
    let result = LinkGenerationService::build_trackable_url(&link, base_url);
    assert_eq!(result, "https://app.example.com/r/xyz789");
}

#[test]
fn test_build_trackable_url_utm_type() {
    let link = CampaignLink {
        id: LinkId::new("test-link"),
        short_code: "utm123".to_string(),
        target_url: "https://example.com/utm-target".to_string(),
        link_type: "utm".to_string(),
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
        is_active: Some(true),
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    let base_url = "https://app.example.com";
    let result = LinkGenerationService::build_trackable_url(&link, base_url);
    assert_eq!(result, "https://example.com/utm-target");
}

#[test]
fn test_build_trackable_url_unknown_type() {
    let link = CampaignLink {
        id: LinkId::new("test-link"),
        short_code: "unk123".to_string(),
        target_url: "https://example.com/unknown".to_string(),
        link_type: "unknown".to_string(),
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
        is_active: Some(true),
        expires_at: None,
        created_at: None,
        updated_at: None,
    };

    let base_url = "https://app.example.com";
    let result = LinkGenerationService::build_trackable_url(&link, base_url);
    assert_eq!(result, "https://example.com/unknown");
}

// ============================================================================
// LinkType Behavior Tests
// ============================================================================

#[test]
fn test_link_type_redirect_in_trackable_url() {
    let link = CampaignLink {
        id: LinkId::new("link"),
        short_code: "redir1".to_string(),
        target_url: "https://example.com".to_string(),
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

    let base = "https://app.com";
    let result = LinkGenerationService::build_trackable_url(&link, base);
    assert!(result.starts_with("https://app.com/r/"));
}

#[test]
fn test_link_type_both_in_trackable_url() {
    let link = CampaignLink {
        id: LinkId::new("link"),
        short_code: "both1".to_string(),
        target_url: "https://example.com".to_string(),
        link_type: "both".to_string(),
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

    let base = "https://app.com";
    let result = LinkGenerationService::build_trackable_url(&link, base);
    assert_eq!(result, "https://app.com/r/both1");
}
