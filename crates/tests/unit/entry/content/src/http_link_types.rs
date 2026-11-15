use systemprompt_content_entry::http::links::{
    AnalyticsQuery, GenerateLinkRequest, GenerateLinkResponse, ListLinksQuery,
};
use systemprompt_identifiers::{CampaignId, ContentId, LinkId};

#[test]
fn generate_link_request_needs_only_target_and_type() {
    let request: GenerateLinkRequest = serde_json::from_str(
        r#"{"target_url": "https://example.com/target", "link_type": "redirect"}"#,
    )
    .unwrap();
    assert_eq!(request.target_url, "https://example.com/target");
    assert_eq!(request.link_type, "redirect");
    assert!(request.campaign_id.is_none());
    assert!(request.source_content_id.is_none());
    assert!(request.utm_source.is_none());
    assert!(request.expires_at.is_none());
}

#[test]
fn generate_link_request_reads_every_optional_field() {
    let request: GenerateLinkRequest = serde_json::from_str(
        r#"{
            "target_url": "https://example.com/page",
            "link_type": "both",
            "campaign_id": "summer-2024",
            "campaign_name": "Summer Sale",
            "source_content_id": "content-123",
            "source_page": "/blog/article",
            "utm_source": "google",
            "utm_medium": "cpc",
            "utm_campaign": "summer",
            "utm_term": "sale",
            "utm_content": "banner",
            "link_text": "Click here",
            "link_position": "header",
            "expires_at": "2025-12-31T23:59:59Z"
        }"#,
    )
    .unwrap();
    assert_eq!(request.campaign_id, Some(CampaignId::new("summer-2024")));
    assert_eq!(request.campaign_name.as_deref(), Some("Summer Sale"));
    assert_eq!(request.source_content_id, Some(ContentId::new("content-123")));
    assert_eq!(request.source_page.as_deref(), Some("/blog/article"));
    assert_eq!(request.utm_source.as_deref(), Some("google"));
    assert_eq!(request.utm_medium.as_deref(), Some("cpc"));
    assert_eq!(request.utm_campaign.as_deref(), Some("summer"));
    assert_eq!(request.utm_term.as_deref(), Some("sale"));
    assert_eq!(request.utm_content.as_deref(), Some("banner"));
    assert_eq!(request.link_text.as_deref(), Some("Click here"));
    assert_eq!(request.link_position.as_deref(), Some("header"));
    assert_eq!(
        request.expires_at.map(|t| t.to_rfc3339()),
        Some("2025-12-31T23:59:59+00:00".to_owned())
    );
}

#[test]
fn generate_link_request_without_a_target_is_rejected() {
    assert!(serde_json::from_str::<GenerateLinkRequest>(r#"{"link_type": "redirect"}"#).is_err());
}

#[test]
fn generate_link_response_serialises_its_four_fields() {
    let response = GenerateLinkResponse {
        link_id: LinkId::new("link-123"),
        short_code: "abc123".to_owned(),
        redirect_url: "https://short.example.com/r/abc123".to_owned(),
        full_url: "https://example.com/target?utm_source=google".to_owned(),
    };
    assert_eq!(
        serde_json::to_value(&response).unwrap(),
        serde_json::json!({
            "link_id": "link-123",
            "short_code": "abc123",
            "redirect_url": "https://short.example.com/r/abc123",
            "full_url": "https://example.com/target?utm_source=google",
        })
    );
}

#[test]
fn list_links_query_filters_are_optional() {
    let empty: ListLinksQuery = serde_json::from_str("{}").unwrap();
    assert!(empty.campaign_id.is_none());
    assert!(empty.source_content_id.is_none());

    let both: ListLinksQuery =
        serde_json::from_str(r#"{"campaign_id": "camp", "source_content_id": "content"}"#).unwrap();
    assert_eq!(both.campaign_id, Some(CampaignId::new("camp")));
    assert_eq!(both.source_content_id, Some(ContentId::new("content")));
}

#[test]
fn analytics_query_paging_is_optional() {
    let empty: AnalyticsQuery = serde_json::from_str("{}").unwrap();
    assert_eq!((empty.limit, empty.offset), (None, None));

    let paged: AnalyticsQuery = serde_json::from_str(r#"{"limit": 25, "offset": 50}"#).unwrap();
    assert_eq!((paged.limit, paged.offset), (Some(25), Some(50)));
}
