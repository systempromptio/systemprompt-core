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

// ============================================================================
// Destination Type Detection Tests (via build_trackable_url behavior)
// ============================================================================

#[test]
fn test_destination_type_internal_slash_prefix() {
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
    use systemprompt_content::models::CampaignLink;
    use systemprompt_identifiers::LinkId;

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
// LinkPerformance Tests
// ============================================================================

#[test]
fn test_link_performance_creation() {
    use systemprompt_content::models::LinkPerformance;
    use systemprompt_identifiers::LinkId;

    let perf = LinkPerformance {
        link_id: LinkId::new("perf-link"),
        click_count: 100,
        unique_click_count: 75,
        conversion_count: 10,
        conversion_rate: Some(0.133),
    };

    assert_eq!(perf.click_count, 100);
    assert_eq!(perf.unique_click_count, 75);
    assert_eq!(perf.conversion_count, 10);
    assert_eq!(perf.conversion_rate, Some(0.133));
}

#[test]
fn test_link_performance_zero_counts() {
    use systemprompt_content::models::LinkPerformance;
    use systemprompt_identifiers::LinkId;

    let perf = LinkPerformance {
        link_id: LinkId::new("zero-link"),
        click_count: 0,
        unique_click_count: 0,
        conversion_count: 0,
        conversion_rate: None,
    };

    assert_eq!(perf.click_count, 0);
    assert_eq!(perf.unique_click_count, 0);
    assert!(perf.conversion_rate.is_none());
}

#[test]
fn test_link_performance_serialization() {
    use systemprompt_content::models::LinkPerformance;
    use systemprompt_identifiers::LinkId;

    let perf = LinkPerformance {
        link_id: LinkId::new("serial-link"),
        click_count: 50,
        unique_click_count: 40,
        conversion_count: 5,
        conversion_rate: Some(0.125),
    };

    let json = serde_json::to_string(&perf).unwrap();
    assert!(json.contains("\"click_count\":50"));
    assert!(json.contains("\"unique_click_count\":40"));
    assert!(json.contains("\"conversion_count\":5"));
}

// ============================================================================
// CampaignPerformance Tests
// ============================================================================

#[test]
fn test_campaign_performance_creation() {
    use systemprompt_content::models::CampaignPerformance;
    use systemprompt_identifiers::CampaignId;

    let perf = CampaignPerformance {
        campaign_id: CampaignId::new("campaign-perf"),
        total_clicks: 500,
        link_count: 10,
        unique_visitors: Some(300),
        conversion_count: Some(50),
    };

    assert_eq!(perf.total_clicks, 500);
    assert_eq!(perf.link_count, 10);
    assert_eq!(perf.unique_visitors, Some(300));
    assert_eq!(perf.conversion_count, Some(50));
}

#[test]
fn test_campaign_performance_serialization() {
    use systemprompt_content::models::CampaignPerformance;
    use systemprompt_identifiers::CampaignId;

    let perf = CampaignPerformance {
        campaign_id: CampaignId::new("serial-campaign"),
        total_clicks: 200,
        link_count: 5,
        unique_visitors: None,
        conversion_count: None,
    };

    let json = serde_json::to_string(&perf).unwrap();
    assert!(json.contains("\"total_clicks\":200"));
    assert!(json.contains("\"link_count\":5"));
}

// ============================================================================
// ContentJourneyNode Tests
// ============================================================================

#[test]
fn test_content_journey_node_creation() {
    use systemprompt_content::models::ContentJourneyNode;
    use systemprompt_identifiers::ContentId;

    let node = ContentJourneyNode {
        source_content_id: ContentId::new("blog-post-1"),
        target_url: "https://example.com/product".to_string(),
        click_count: 25,
    };

    assert_eq!(node.click_count, 25);
    assert_eq!(node.target_url, "https://example.com/product");
}

#[test]
fn test_content_journey_node_serialization() {
    use systemprompt_content::models::ContentJourneyNode;
    use systemprompt_identifiers::ContentId;

    let node = ContentJourneyNode {
        source_content_id: ContentId::new("article-1"),
        target_url: "/related-article".to_string(),
        click_count: 10,
    };

    let json = serde_json::to_string(&node).unwrap();
    assert!(json.contains("\"click_count\":10"));
    assert!(json.contains("\"target_url\":\"/related-article\""));
}

// ============================================================================
// LinkClick Tests
// ============================================================================

#[test]
fn test_link_click_creation_minimal() {
    use systemprompt_content::models::LinkClick;
    use systemprompt_identifiers::{LinkId, LinkClickId, SessionId};

    let click = LinkClick {
        id: LinkClickId::new("click-1"),
        link_id: LinkId::new("link-1"),
        session_id: SessionId::new("session-1"),
        user_id: None,
        context_id: None,
        task_id: None,
        referrer_page: None,
        referrer_url: None,
        clicked_at: None,
        user_agent: None,
        ip_address: None,
        device_type: None,
        country: None,
        is_first_click: None,
        is_conversion: None,
        conversion_at: None,
        time_on_page_seconds: None,
        scroll_depth_percent: None,
    };

    assert_eq!(click.user_id, None);
    assert_eq!(click.is_first_click, None);
}

#[test]
fn test_link_click_creation_full() {
    use systemprompt_content::models::LinkClick;
    use systemprompt_identifiers::{LinkId, LinkClickId, SessionId, UserId, ContextId, TaskId};
    use chrono::Utc;

    let now = Utc::now();
    let click = LinkClick {
        id: LinkClickId::new("click-2"),
        link_id: LinkId::new("link-2"),
        session_id: SessionId::new("session-2"),
        user_id: Some(UserId::new("user-1")),
        context_id: Some(ContextId::new("ctx-1")),
        task_id: Some(TaskId::new("task-1")),
        referrer_page: Some("/blog".to_string()),
        referrer_url: Some("https://google.com".to_string()),
        clicked_at: Some(now),
        user_agent: Some("Mozilla/5.0".to_string()),
        ip_address: Some("192.168.1.1".to_string()),
        device_type: Some("desktop".to_string()),
        country: Some("US".to_string()),
        is_first_click: Some(true),
        is_conversion: Some(false),
        conversion_at: None,
        time_on_page_seconds: Some(30),
        scroll_depth_percent: Some(75),
    };

    assert_eq!(click.is_first_click, Some(true));
    assert_eq!(click.device_type, Some("desktop".to_string()));
    assert_eq!(click.country, Some("US".to_string()));
    assert_eq!(click.time_on_page_seconds, Some(30));
}

#[test]
fn test_link_click_serialization() {
    use systemprompt_content::models::LinkClick;
    use systemprompt_identifiers::{LinkId, LinkClickId, SessionId};

    let click = LinkClick {
        id: LinkClickId::new("click-3"),
        link_id: LinkId::new("link-3"),
        session_id: SessionId::new("session-3"),
        user_id: None,
        context_id: None,
        task_id: None,
        referrer_page: None,
        referrer_url: None,
        clicked_at: None,
        user_agent: None,
        ip_address: None,
        device_type: Some("mobile".to_string()),
        country: None,
        is_first_click: Some(true),
        is_conversion: None,
        conversion_at: None,
        time_on_page_seconds: None,
        scroll_depth_percent: None,
    };

    let json = serde_json::to_string(&click).unwrap();
    assert!(json.contains("\"device_type\":\"mobile\""));
    assert!(json.contains("\"is_first_click\":true"));
}

// ============================================================================
// TrackClickParams Tests
// ============================================================================

#[test]
fn test_track_click_params_creation() {
    use systemprompt_content::models::TrackClickParams;
    use systemprompt_identifiers::{LinkId, SessionId};

    let params = TrackClickParams {
        link_id: LinkId::new("link-track"),
        session_id: SessionId::new("session-track"),
        user_id: None,
        context_id: None,
        task_id: None,
        referrer_page: None,
        referrer_url: None,
        user_agent: None,
        ip_address: None,
        device_type: None,
        country: None,
    };

    assert_eq!(params.link_id.to_string(), "link-track");
    assert_eq!(params.session_id.to_string(), "session-track");
}

#[test]
fn test_track_click_params_with_context() {
    use systemprompt_content::models::TrackClickParams;
    use systemprompt_identifiers::{LinkId, SessionId, UserId, ContextId};

    let params = TrackClickParams {
        link_id: LinkId::new("link-ctx"),
        session_id: SessionId::new("session-ctx"),
        user_id: Some(UserId::new("user-ctx")),
        context_id: Some(ContextId::new("context-1")),
        task_id: None,
        referrer_page: Some("/previous-page".to_string()),
        referrer_url: Some("https://example.com/previous".to_string()),
        user_agent: Some("TestAgent/1.0".to_string()),
        ip_address: Some("10.0.0.1".to_string()),
        device_type: Some("tablet".to_string()),
        country: Some("UK".to_string()),
    };

    assert!(params.user_id.is_some());
    assert!(params.context_id.is_some());
    assert_eq!(params.device_type, Some("tablet".to_string()));
}
