//! Unit tests for link builder patterns
//!
//! Tests cover:
//! - CreateLinkParams builder
//! - RecordClickParams builder
//! - TrackClickParams builder

use chrono::{TimeZone, Utc};
use systemprompt_core_content::models::{CreateLinkParams, RecordClickParams, TrackClickParams};
use systemprompt_identifiers::{
    CampaignId, ContentId, ContextId, LinkClickId, LinkId, SessionId, TaskId, UserId,
};

// ============================================================================
// CreateLinkParams Tests
// ============================================================================

#[test]
fn test_create_link_params_new() {
    let params = CreateLinkParams::new(
        "abc123".to_string(),
        "https://example.com/target".to_string(),
        "redirect".to_string(),
    );

    assert_eq!(params.short_code, "abc123");
    assert_eq!(params.target_url, "https://example.com/target");
    assert_eq!(params.link_type, "redirect");
    assert!(params.source_content_id.is_none());
    assert!(params.source_page.is_none());
    assert!(params.campaign_id.is_none());
    assert!(params.campaign_name.is_none());
    assert!(params.utm_params.is_none());
    assert!(params.link_text.is_none());
    assert!(params.link_position.is_none());
    assert!(params.destination_type.is_none());
    assert!(params.is_active);
    assert!(params.expires_at.is_none());
}

#[test]
fn test_create_link_params_with_source_content_id() {
    let params = CreateLinkParams::new("code".to_string(), "url".to_string(), "utm".to_string())
        .with_source_content_id(Some(ContentId::new("content-123")));

    assert_eq!(
        params.source_content_id.as_ref().unwrap().as_str(),
        "content-123"
    );
}

#[test]
fn test_create_link_params_with_source_page() {
    let params = CreateLinkParams::new("code".to_string(), "url".to_string(), "utm".to_string())
        .with_source_page(Some("/blog/article".to_string()));

    assert_eq!(params.source_page, Some("/blog/article".to_string()));
}

#[test]
fn test_create_link_params_with_campaign_id() {
    let params = CreateLinkParams::new("code".to_string(), "url".to_string(), "both".to_string())
        .with_campaign_id(Some(CampaignId::new("summer-2024")));

    assert_eq!(params.campaign_id.as_ref().unwrap().as_str(), "summer-2024");
}

#[test]
fn test_create_link_params_with_campaign_name() {
    let params = CreateLinkParams::new("code".to_string(), "url".to_string(), "both".to_string())
        .with_campaign_name(Some("Summer Sale 2024".to_string()));

    assert_eq!(params.campaign_name, Some("Summer Sale 2024".to_string()));
}

#[test]
fn test_create_link_params_with_utm_params() {
    let utm_json = r#"{"source":"google","medium":"cpc"}"#;
    let params = CreateLinkParams::new("code".to_string(), "url".to_string(), "utm".to_string())
        .with_utm_params(Some(utm_json.to_string()));

    assert_eq!(params.utm_params, Some(utm_json.to_string()));
}

#[test]
fn test_create_link_params_with_link_text() {
    let params = CreateLinkParams::new("code".to_string(), "url".to_string(), "redirect".to_string())
        .with_link_text(Some("Click here".to_string()));

    assert_eq!(params.link_text, Some("Click here".to_string()));
}

#[test]
fn test_create_link_params_with_link_position() {
    let params = CreateLinkParams::new("code".to_string(), "url".to_string(), "redirect".to_string())
        .with_link_position(Some("header".to_string()));

    assert_eq!(params.link_position, Some("header".to_string()));
}

#[test]
fn test_create_link_params_with_destination_type() {
    let params = CreateLinkParams::new("code".to_string(), "url".to_string(), "redirect".to_string())
        .with_destination_type(Some("external".to_string()));

    assert_eq!(params.destination_type, Some("external".to_string()));
}

#[test]
fn test_create_link_params_with_is_active() {
    let params = CreateLinkParams::new("code".to_string(), "url".to_string(), "redirect".to_string())
        .with_is_active(false);

    assert!(!params.is_active);
}

#[test]
fn test_create_link_params_with_expires_at() {
    let expires = Utc.with_ymd_and_hms(2025, 12, 31, 23, 59, 59).unwrap();
    let params = CreateLinkParams::new("code".to_string(), "url".to_string(), "redirect".to_string())
        .with_expires_at(Some(expires));

    assert_eq!(params.expires_at, Some(expires));
}

#[test]
fn test_create_link_params_full_builder() {
    let expires = Utc.with_ymd_and_hms(2025, 6, 30, 0, 0, 0).unwrap();
    let params = CreateLinkParams::new(
        "fullcode".to_string(),
        "https://example.com/full".to_string(),
        "both".to_string(),
    )
    .with_source_content_id(Some(ContentId::new("src-content")))
    .with_source_page(Some("/page".to_string()))
    .with_campaign_id(Some(CampaignId::new("camp-id")))
    .with_campaign_name(Some("Campaign".to_string()))
    .with_utm_params(Some("{}".to_string()))
    .with_link_text(Some("Text".to_string()))
    .with_link_position(Some("footer".to_string()))
    .with_destination_type(Some("internal".to_string()))
    .with_is_active(true)
    .with_expires_at(Some(expires));

    assert_eq!(params.short_code, "fullcode");
    assert!(params.source_content_id.is_some());
    assert!(params.source_page.is_some());
    assert!(params.campaign_id.is_some());
    assert!(params.campaign_name.is_some());
    assert!(params.utm_params.is_some());
    assert!(params.link_text.is_some());
    assert!(params.link_position.is_some());
    assert!(params.destination_type.is_some());
    assert!(params.is_active);
    assert!(params.expires_at.is_some());
}

// ============================================================================
// RecordClickParams Tests
// ============================================================================

#[test]
fn test_record_click_params_new() {
    let clicked_at = Utc::now();
    let params = RecordClickParams::new(
        LinkClickId::generate(),
        LinkId::new("link-1"),
        SessionId::new("session-1"),
        clicked_at,
    );

    assert_eq!(params.link_id.as_str(), "link-1");
    assert_eq!(params.session_id.as_str(), "session-1");
    assert_eq!(params.clicked_at, clicked_at);
    assert!(params.user_id.is_none());
    assert!(params.context_id.is_none());
    assert!(params.task_id.is_none());
    assert!(params.referrer_page.is_none());
    assert!(params.referrer_url.is_none());
    assert!(params.user_agent.is_none());
    assert!(params.ip_address.is_none());
    assert!(params.device_type.is_none());
    assert!(params.country.is_none());
    assert!(!params.is_first_click);
    assert!(!params.is_conversion);
}

#[test]
fn test_record_click_params_with_user_id() {
    let clicked_at = Utc::now();
    let params = RecordClickParams::new(
        LinkClickId::generate(),
        LinkId::new("link"),
        SessionId::new("session"),
        clicked_at,
    )
    .with_user_id(Some(UserId::new("user-123")));

    assert_eq!(params.user_id.as_ref().unwrap().as_str(), "user-123");
}

#[test]
fn test_record_click_params_with_context_id() {
    let clicked_at = Utc::now();
    let params = RecordClickParams::new(
        LinkClickId::generate(),
        LinkId::new("link"),
        SessionId::new("session"),
        clicked_at,
    )
    .with_context_id(Some(ContextId::new("ctx-456")));

    assert_eq!(params.context_id.as_ref().unwrap().as_str(), "ctx-456");
}

#[test]
fn test_record_click_params_with_task_id() {
    let clicked_at = Utc::now();
    let params = RecordClickParams::new(
        LinkClickId::generate(),
        LinkId::new("link"),
        SessionId::new("session"),
        clicked_at,
    )
    .with_task_id(Some(TaskId::new("task-789")));

    assert_eq!(params.task_id.as_ref().unwrap().as_str(), "task-789");
}

#[test]
fn test_record_click_params_with_referrer() {
    let clicked_at = Utc::now();
    let params = RecordClickParams::new(
        LinkClickId::generate(),
        LinkId::new("link"),
        SessionId::new("session"),
        clicked_at,
    )
    .with_referrer_page(Some("/home".to_string()))
    .with_referrer_url(Some("https://example.com/home".to_string()));

    assert_eq!(params.referrer_page, Some("/home".to_string()));
    assert_eq!(
        params.referrer_url,
        Some("https://example.com/home".to_string())
    );
}

#[test]
fn test_record_click_params_with_user_agent() {
    let clicked_at = Utc::now();
    let params = RecordClickParams::new(
        LinkClickId::generate(),
        LinkId::new("link"),
        SessionId::new("session"),
        clicked_at,
    )
    .with_user_agent(Some("Mozilla/5.0".to_string()));

    assert_eq!(params.user_agent, Some("Mozilla/5.0".to_string()));
}

#[test]
fn test_record_click_params_with_ip_address() {
    let clicked_at = Utc::now();
    let params = RecordClickParams::new(
        LinkClickId::generate(),
        LinkId::new("link"),
        SessionId::new("session"),
        clicked_at,
    )
    .with_ip_address(Some("192.168.1.1".to_string()));

    assert_eq!(params.ip_address, Some("192.168.1.1".to_string()));
}

#[test]
fn test_record_click_params_with_device_type() {
    let clicked_at = Utc::now();
    let params = RecordClickParams::new(
        LinkClickId::generate(),
        LinkId::new("link"),
        SessionId::new("session"),
        clicked_at,
    )
    .with_device_type(Some("mobile".to_string()));

    assert_eq!(params.device_type, Some("mobile".to_string()));
}

#[test]
fn test_record_click_params_with_country() {
    let clicked_at = Utc::now();
    let params = RecordClickParams::new(
        LinkClickId::generate(),
        LinkId::new("link"),
        SessionId::new("session"),
        clicked_at,
    )
    .with_country(Some("US".to_string()));

    assert_eq!(params.country, Some("US".to_string()));
}

#[test]
fn test_record_click_params_with_is_first_click() {
    let clicked_at = Utc::now();
    let params = RecordClickParams::new(
        LinkClickId::generate(),
        LinkId::new("link"),
        SessionId::new("session"),
        clicked_at,
    )
    .with_is_first_click(true);

    assert!(params.is_first_click);
}

#[test]
fn test_record_click_params_with_is_conversion() {
    let clicked_at = Utc::now();
    let params = RecordClickParams::new(
        LinkClickId::generate(),
        LinkId::new("link"),
        SessionId::new("session"),
        clicked_at,
    )
    .with_is_conversion(true);

    assert!(params.is_conversion);
}

// ============================================================================
// TrackClickParams Tests
// ============================================================================

#[test]
fn test_track_click_params_new() {
    let params = TrackClickParams::new(LinkId::new("track-link"), SessionId::new("track-session"));

    assert_eq!(params.link_id.as_str(), "track-link");
    assert_eq!(params.session_id.as_str(), "track-session");
    assert!(params.user_id.is_none());
    assert!(params.context_id.is_none());
    assert!(params.task_id.is_none());
    assert!(params.referrer_page.is_none());
    assert!(params.referrer_url.is_none());
    assert!(params.user_agent.is_none());
    assert!(params.ip_address.is_none());
    assert!(params.device_type.is_none());
    assert!(params.country.is_none());
}

#[test]
fn test_track_click_params_with_user_id() {
    let params = TrackClickParams::new(LinkId::new("link"), SessionId::new("session"))
        .with_user_id(Some(UserId::new("user")));

    assert_eq!(params.user_id.as_ref().unwrap().as_str(), "user");
}

#[test]
fn test_track_click_params_with_context_id() {
    let params = TrackClickParams::new(LinkId::new("link"), SessionId::new("session"))
        .with_context_id(Some(ContextId::new("ctx")));

    assert_eq!(params.context_id.as_ref().unwrap().as_str(), "ctx");
}

#[test]
fn test_track_click_params_with_task_id() {
    let params = TrackClickParams::new(LinkId::new("link"), SessionId::new("session"))
        .with_task_id(Some(TaskId::new("task")));

    assert_eq!(params.task_id.as_ref().unwrap().as_str(), "task");
}

#[test]
fn test_track_click_params_full_builder() {
    let params = TrackClickParams::new(LinkId::new("full-link"), SessionId::new("full-session"))
        .with_user_id(Some(UserId::new("u")))
        .with_context_id(Some(ContextId::new("c")))
        .with_task_id(Some(TaskId::new("t")))
        .with_referrer_page(Some("/ref".to_string()))
        .with_referrer_url(Some("https://ref.com".to_string()))
        .with_user_agent(Some("Agent".to_string()))
        .with_ip_address(Some("1.2.3.4".to_string()))
        .with_device_type(Some("desktop".to_string()))
        .with_country(Some("UK".to_string()));

    assert!(params.user_id.is_some());
    assert!(params.context_id.is_some());
    assert!(params.task_id.is_some());
    assert!(params.referrer_page.is_some());
    assert!(params.referrer_url.is_some());
    assert!(params.user_agent.is_some());
    assert!(params.ip_address.is_some());
    assert!(params.device_type.is_some());
    assert!(params.country.is_some());
}
