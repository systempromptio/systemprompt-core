//! Unit tests for link services
//!
//! Tests cover:
//! - LinkGenerationService static methods
//! - build_trackable_url
//! - inject_utm_params
//! - determine_destination_type (internal behavior)

use systemprompt_content::models::UtmParams;
use systemprompt_content::services::LinkGenerationService;

// ============================================================================
// inject_utm_params Tests
// ============================================================================

#[test]
fn test_inject_utm_params_empty_params() {
    let params = UtmParams {
        source: None,
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "https://example.com/page";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert_eq!(result, url);
}

#[test]
fn test_inject_utm_params_single_param() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "https://example.com/page";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert_eq!(result, "https://example.com/page?utm_source=google");
}

#[test]
fn test_inject_utm_params_multiple_params() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: Some("cpc".to_string()),
        campaign: Some("summer".to_string()),
        term: None,
        content: None,
    };

    let url = "https://example.com/page";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert!(result.contains("utm_source=google"));
    assert!(result.contains("utm_medium=cpc"));
    assert!(result.contains("utm_campaign=summer"));
    assert!(result.starts_with("https://example.com/page?"));
}

#[test]
fn test_inject_utm_params_all_params() {
    let params = UtmParams {
        source: Some("source".to_string()),
        medium: Some("medium".to_string()),
        campaign: Some("campaign".to_string()),
        term: Some("term".to_string()),
        content: Some("content".to_string()),
    };

    let url = "https://example.com";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert!(result.contains("utm_source=source"));
    assert!(result.contains("utm_medium=medium"));
    assert!(result.contains("utm_campaign=campaign"));
    assert!(result.contains("utm_term=term"));
    assert!(result.contains("utm_content=content"));
}

#[test]
fn test_inject_utm_params_url_with_existing_query() {
    let params = UtmParams {
        source: Some("twitter".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "https://example.com/page?existing=param";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert_eq!(
        result,
        "https://example.com/page?existing=param&utm_source=twitter"
    );
}

#[test]
fn test_inject_utm_params_url_with_fragment() {
    let params = UtmParams {
        source: Some("email".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    // Note: This test documents current behavior; fragment handling may vary
    let url = "https://example.com/page#section";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert!(result.contains("utm_source=email"));
}

// ============================================================================
// build_trackable_url Tests
// ============================================================================

#[test]
fn test_build_trackable_url_redirect_type() {
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

    // Create a minimal CampaignLink for testing
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
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    // UTM type returns target_url directly
    assert_eq!(result, "https://example.com/utm-target");
}

#[test]
fn test_build_trackable_url_unknown_type() {
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    // Unknown type returns target_url directly
    assert_eq!(result, "https://example.com/unknown");
}

// ============================================================================
// CampaignLink.get_full_url Tests
// ============================================================================

#[test]
fn test_campaign_link_get_full_url_no_utm() {
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    // Should use & not ? for additional params
    assert!(result.contains("&utm_source"));
}

#[test]
fn test_campaign_link_get_full_url_invalid_utm_json() {
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    // Invalid JSON should return original URL
    assert_eq!(result, "https://example.com/page");
}

// ============================================================================
// GenerateLinkParams Tests
// ============================================================================

#[test]
fn test_generate_link_params_debug() {
    use systemprompt_content::services::link::generation::GenerateLinkParams;
    use systemprompt_content::models::LinkType;

    let params = GenerateLinkParams {
        target_url: "https://example.com".to_string(),
        link_type: LinkType::Redirect,
        campaign_id: None,
        campaign_name: None,
        source_content_id: None,
        source_page: None,
        utm_params: None,
        link_text: None,
        link_position: None,
        expires_at: None,
    };

    let debug = format!("{:?}", params);
    assert!(debug.contains("GenerateLinkParams"));
    assert!(debug.contains("target_url"));
}

#[test]
fn test_generate_link_params_full() {
    use systemprompt_content::services::link::generation::GenerateLinkParams;
    use systemprompt_content::models::{LinkType, UtmParams};
    use systemprompt_identifiers::{CampaignId, ContentId};
    use chrono::Utc;

    let params = GenerateLinkParams {
        target_url: "https://example.com/target".to_string(),
        link_type: LinkType::Both,
        campaign_id: Some(CampaignId::new("campaign-123")),
        campaign_name: Some("Test Campaign".to_string()),
        source_content_id: Some(ContentId::new("content-456")),
        source_page: Some("/blog/article".to_string()),
        utm_params: Some(UtmParams {
            source: Some("google".to_string()),
            medium: Some("cpc".to_string()),
            campaign: Some("test".to_string()),
            term: None,
            content: None,
        }),
        link_text: Some("Click Here".to_string()),
        link_position: Some("header".to_string()),
        expires_at: Some(Utc::now()),
    };

    assert_eq!(params.target_url, "https://example.com/target");
    assert!(matches!(params.link_type, LinkType::Both));
    assert!(params.campaign_id.is_some());
    assert!(params.utm_params.is_some());
    assert!(params.expires_at.is_some());
}

// ============================================================================
// GenerateContentLinkParams Tests
// ============================================================================

#[test]
fn test_generate_content_link_params_debug() {
    use systemprompt_content::services::link::generation::GenerateContentLinkParams;
    use systemprompt_identifiers::ContentId;

    let content_id = ContentId::new("content-123");
    let params = GenerateContentLinkParams {
        target_url: "https://example.com",
        source_content_id: &content_id,
        source_page: "/blog",
        link_text: None,
        link_position: None,
    };

    let debug = format!("{:?}", params);
    assert!(debug.contains("GenerateContentLinkParams"));
}

#[test]
fn test_generate_content_link_params_full() {
    use systemprompt_content::services::link::generation::GenerateContentLinkParams;
    use systemprompt_identifiers::ContentId;

    let content_id = ContentId::new("content-456");
    let params = GenerateContentLinkParams {
        target_url: "https://example.com/target",
        source_content_id: &content_id,
        source_page: "/blog/article",
        link_text: Some("Read More".to_string()),
        link_position: Some("footer".to_string()),
    };

    assert_eq!(params.target_url, "https://example.com/target");
    assert_eq!(params.source_page, "/blog/article");
    assert_eq!(params.link_text, Some("Read More".to_string()));
    assert_eq!(params.link_position, Some("footer".to_string()));
}

// ============================================================================
// LinkType Behavior Tests
// ============================================================================

#[test]
fn test_link_type_redirect_in_trackable_url() {
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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

// ============================================================================
// Edge Cases for inject_utm_params
// ============================================================================

#[test]
fn test_inject_utm_params_special_characters() {
    let params = UtmParams {
        source: Some("email+newsletter".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "https://example.com";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert!(result.contains("utm_source=email%2Bnewsletter") || result.contains("utm_source=email+newsletter"));
}

#[test]
fn test_inject_utm_params_empty_url() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    assert!(result.contains("utm_source=google"));
}

#[test]
fn test_inject_utm_params_url_only_query_mark() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let url = "https://example.com?";
    let result = LinkGenerationService::inject_utm_params(url, &params);
    // URL already has ?, so should add with &
    assert!(result.contains("&utm_source=google"));
}

// ============================================================================
// UtmParams Tests
// ============================================================================

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
fn test_utm_params_to_query_string_single() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: None,
        campaign: None,
        term: None,
        content: None,
    };

    let query = params.to_query_string();
    assert_eq!(query, "utm_source=google");
}

#[test]
fn test_utm_params_to_query_string_multiple() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: Some("cpc".to_string()),
        campaign: None,
        term: None,
        content: None,
    };

    let query = params.to_query_string();
    assert!(query.contains("utm_source=google"));
    assert!(query.contains("utm_medium=cpc"));
    assert!(query.contains("&"));
}

#[test]
fn test_utm_params_to_json() {
    let params = UtmParams {
        source: Some("google".to_string()),
        medium: Some("cpc".to_string()),
        campaign: Some("summer".to_string()),
        term: None,
        content: None,
    };

    let json = params.to_json().unwrap();
    assert!(json.contains("\"source\":\"google\""));
    assert!(json.contains("\"medium\":\"cpc\""));
    assert!(json.contains("\"campaign\":\"summer\""));
}

#[test]
fn test_utm_params_deserialize() {
    let json = r#"{"source":"twitter","medium":"social","campaign":"winter"}"#;
    let params: UtmParams = serde_json::from_str(json).unwrap();

    assert_eq!(params.source, Some("twitter".to_string()));
    assert_eq!(params.medium, Some("social".to_string()));
    assert_eq!(params.campaign, Some("winter".to_string()));
    assert!(params.term.is_none());
    assert!(params.content.is_none());
}

#[test]
fn test_utm_params_deserialize_empty() {
    let json = r#"{}"#;
    let params: UtmParams = serde_json::from_str(json).unwrap();

    assert!(params.source.is_none());
    assert!(params.medium.is_none());
    assert!(params.campaign.is_none());
    assert!(params.term.is_none());
    assert!(params.content.is_none());
}

#[test]
fn test_utm_params_deserialize_invalid() {
    let json = "not valid json";
    let result: Result<UtmParams, _> = serde_json::from_str(json);
    assert!(result.is_err());
}
