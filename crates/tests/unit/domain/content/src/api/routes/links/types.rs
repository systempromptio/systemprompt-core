//! Unit tests for link API types
//!
//! Tests cover:
//! - GenerateLinkRequest deserialization
//! - GenerateLinkResponse serialization
//! - ListLinksQuery deserialization
//! - AnalyticsQuery deserialization

use systemprompt_core_content::api::routes::links::{
    AnalyticsQuery, GenerateLinkRequest, GenerateLinkResponse, ListLinksQuery,
};

// ============================================================================
// GenerateLinkRequest Tests
// ============================================================================

#[test]
fn test_generate_link_request_minimal() {
    let json = r#"{
        "target_url": "https://example.com/target",
        "link_type": "redirect"
    }"#;

    let request: GenerateLinkRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.target_url, "https://example.com/target");
    assert_eq!(request.link_type, "redirect");
    assert!(request.campaign_id.is_none());
    assert!(request.campaign_name.is_none());
    assert!(request.source_content_id.is_none());
    assert!(request.source_page.is_none());
    assert!(request.utm_source.is_none());
    assert!(request.utm_medium.is_none());
    assert!(request.utm_campaign.is_none());
    assert!(request.utm_term.is_none());
    assert!(request.utm_content.is_none());
    assert!(request.link_text.is_none());
    assert!(request.link_position.is_none());
    assert!(request.expires_at.is_none());
}

#[test]
fn test_generate_link_request_full() {
    let json = r#"{
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
    }"#;

    let request: GenerateLinkRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.target_url, "https://example.com/page");
    assert_eq!(request.link_type, "both");
    assert_eq!(request.campaign_id, Some("summer-2024".to_string()));
    assert_eq!(request.campaign_name, Some("Summer Sale".to_string()));
    assert_eq!(request.source_content_id, Some("content-123".to_string()));
    assert_eq!(request.source_page, Some("/blog/article".to_string()));
    assert_eq!(request.utm_source, Some("google".to_string()));
    assert_eq!(request.utm_medium, Some("cpc".to_string()));
    assert_eq!(request.utm_campaign, Some("summer".to_string()));
    assert_eq!(request.utm_term, Some("sale".to_string()));
    assert_eq!(request.utm_content, Some("banner".to_string()));
    assert_eq!(request.link_text, Some("Click here".to_string()));
    assert_eq!(request.link_position, Some("header".to_string()));
    assert!(request.expires_at.is_some());
}

#[test]
fn test_generate_link_request_with_utm_only() {
    let json = r#"{
        "target_url": "https://example.com",
        "link_type": "utm",
        "utm_source": "twitter",
        "utm_medium": "social"
    }"#;

    let request: GenerateLinkRequest = serde_json::from_str(json).unwrap();
    assert_eq!(request.link_type, "utm");
    assert_eq!(request.utm_source, Some("twitter".to_string()));
    assert_eq!(request.utm_medium, Some("social".to_string()));
    assert!(request.utm_campaign.is_none());
}

#[test]
fn test_generate_link_request_debug() {
    let json = r#"{
        "target_url": "https://example.com",
        "link_type": "redirect"
    }"#;

    let request: GenerateLinkRequest = serde_json::from_str(json).unwrap();
    let debug = format!("{:?}", request);
    assert!(debug.contains("GenerateLinkRequest"));
    assert!(debug.contains("target_url"));
}

// ============================================================================
// GenerateLinkResponse Tests
// ============================================================================

#[test]
fn test_generate_link_response_serialization() {
    let response = GenerateLinkResponse {
        link_id: "link-123".to_string(),
        short_code: "abc123".to_string(),
        redirect_url: "https://short.example.com/r/abc123".to_string(),
        full_url: "https://example.com/target?utm_source=google".to_string(),
    };

    let json = serde_json::to_string(&response).unwrap();
    assert!(json.contains("\"link_id\":\"link-123\""));
    assert!(json.contains("\"short_code\":\"abc123\""));
    assert!(json.contains("\"redirect_url\":\"https://short.example.com/r/abc123\""));
    assert!(json.contains("\"full_url\":"));
}

#[test]
fn test_generate_link_response_fields() {
    let response = GenerateLinkResponse {
        link_id: "test-id".to_string(),
        short_code: "testcode".to_string(),
        redirect_url: "https://r.example.com/testcode".to_string(),
        full_url: "https://example.com/page".to_string(),
    };

    assert_eq!(response.link_id, "test-id");
    assert_eq!(response.short_code, "testcode");
    assert_eq!(response.redirect_url, "https://r.example.com/testcode");
    assert_eq!(response.full_url, "https://example.com/page");
}

#[test]
fn test_generate_link_response_debug() {
    let response = GenerateLinkResponse {
        link_id: "id".to_string(),
        short_code: "code".to_string(),
        redirect_url: "url".to_string(),
        full_url: "full".to_string(),
    };

    let debug = format!("{:?}", response);
    assert!(debug.contains("GenerateLinkResponse"));
}

// ============================================================================
// ListLinksQuery Tests
// ============================================================================

#[test]
fn test_list_links_query_empty() {
    let json = "{}";
    let query: ListLinksQuery = serde_json::from_str(json).unwrap();
    assert!(query.campaign_id.is_none());
    assert!(query.source_content_id.is_none());
}

#[test]
fn test_list_links_query_with_campaign_id() {
    let json = r#"{"campaign_id": "camp-123"}"#;
    let query: ListLinksQuery = serde_json::from_str(json).unwrap();
    assert_eq!(query.campaign_id, Some("camp-123".to_string()));
    assert!(query.source_content_id.is_none());
}

#[test]
fn test_list_links_query_with_source_content_id() {
    let json = r#"{"source_content_id": "content-456"}"#;
    let query: ListLinksQuery = serde_json::from_str(json).unwrap();
    assert!(query.campaign_id.is_none());
    assert_eq!(query.source_content_id, Some("content-456".to_string()));
}

#[test]
fn test_list_links_query_with_both() {
    let json = r#"{
        "campaign_id": "camp",
        "source_content_id": "content"
    }"#;
    let query: ListLinksQuery = serde_json::from_str(json).unwrap();
    assert_eq!(query.campaign_id, Some("camp".to_string()));
    assert_eq!(query.source_content_id, Some("content".to_string()));
}

#[test]
fn test_list_links_query_debug() {
    let query = ListLinksQuery {
        campaign_id: Some("test".to_string()),
        source_content_id: None,
    };
    let debug = format!("{:?}", query);
    assert!(debug.contains("ListLinksQuery"));
}

// ============================================================================
// AnalyticsQuery Tests
// ============================================================================

#[test]
fn test_analytics_query_empty() {
    let json = "{}";
    let query: AnalyticsQuery = serde_json::from_str(json).unwrap();
    assert!(query.limit.is_none());
    assert!(query.offset.is_none());
}

#[test]
fn test_analytics_query_with_limit() {
    let json = r#"{"limit": 50}"#;
    let query: AnalyticsQuery = serde_json::from_str(json).unwrap();
    assert_eq!(query.limit, Some(50));
    assert!(query.offset.is_none());
}

#[test]
fn test_analytics_query_with_offset() {
    let json = r#"{"offset": 100}"#;
    let query: AnalyticsQuery = serde_json::from_str(json).unwrap();
    assert!(query.limit.is_none());
    assert_eq!(query.offset, Some(100));
}

#[test]
fn test_analytics_query_with_both() {
    let json = r#"{"limit": 25, "offset": 50}"#;
    let query: AnalyticsQuery = serde_json::from_str(json).unwrap();
    assert_eq!(query.limit, Some(25));
    assert_eq!(query.offset, Some(50));
}

#[test]
fn test_analytics_query_copy() {
    let query = AnalyticsQuery {
        limit: Some(10),
        offset: Some(20),
    };
    let copied = query;
    assert_eq!(copied.limit, Some(10));
    assert_eq!(copied.offset, Some(20));
}

#[test]
fn test_analytics_query_debug() {
    let query = AnalyticsQuery {
        limit: Some(10),
        offset: None,
    };
    let debug = format!("{:?}", query);
    assert!(debug.contains("AnalyticsQuery"));
}

#[test]
fn test_analytics_query_clone() {
    let query = AnalyticsQuery {
        limit: Some(5),
        offset: Some(10),
    };
    let cloned = query;
    assert_eq!(cloned.limit, query.limit);
    assert_eq!(cloned.offset, query.offset);
}

// ============================================================================
// Edge Case Tests
// ============================================================================

#[test]
fn test_generate_link_request_empty_strings() {
    let json = r#"{
        "target_url": "",
        "link_type": ""
    }"#;

    let request: GenerateLinkRequest = serde_json::from_str(json).unwrap();
    assert!(request.target_url.is_empty());
    assert!(request.link_type.is_empty());
}

#[test]
fn test_analytics_query_zero_values() {
    let json = r#"{"limit": 0, "offset": 0}"#;
    let query: AnalyticsQuery = serde_json::from_str(json).unwrap();
    assert_eq!(query.limit, Some(0));
    assert_eq!(query.offset, Some(0));
}

#[test]
fn test_analytics_query_large_values() {
    let json = r#"{"limit": 9999999, "offset": 9999999}"#;
    let query: AnalyticsQuery = serde_json::from_str(json).unwrap();
    assert_eq!(query.limit, Some(9999999));
    assert_eq!(query.offset, Some(9999999));
}

#[test]
fn test_generate_link_request_special_characters() {
    let json = r#"{
        "target_url": "https://example.com/path?param=value&other=test",
        "link_type": "redirect",
        "link_text": "Click here <button>",
        "campaign_name": "Summer Sale 2024 - 50% Off!"
    }"#;

    let request: GenerateLinkRequest = serde_json::from_str(json).unwrap();
    assert!(request.target_url.contains("?"));
    assert!(request.target_url.contains("&"));
    assert!(request.link_text.unwrap().contains("<button>"));
    assert!(request.campaign_name.unwrap().contains("50%"));
}

#[test]
fn test_generate_link_request_unicode() {
    let json = r#"{
        "target_url": "https://example.com/path",
        "link_type": "redirect",
        "link_text": "Click here ",
        "campaign_name": ""
    }"#;

    let request: GenerateLinkRequest = serde_json::from_str(json).unwrap();
    assert!(request.link_text.unwrap().contains(""));
    assert!(request.campaign_name.unwrap().contains(""));
}
