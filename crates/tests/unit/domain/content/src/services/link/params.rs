//! Tests for GenerateLinkParams and GenerateContentLinkParams

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
