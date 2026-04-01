//! Tests for LinkPerformance, CampaignPerformance, ContentJourneyNode, LinkClick, and TrackClickParams

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
