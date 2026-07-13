//! Tests for `SessionAnalytics::from_request` (the axum `Request` entry point
//! that folds headers, URI UTM params, and content routing) and the
//! `should_skip_tracking` AI-crawler exemption branch.

use axum::body::Body;
use axum::extract::Request;
use axum::http::HeaderValue;
use systemprompt_analytics::SessionAnalytics;
use systemprompt_models::ContentRouting;

#[derive(Debug)]
struct HtmlRouting;

impl ContentRouting for HtmlRouting {
    fn is_html_page(&self, _path: &str) -> bool {
        true
    }
    fn determine_source(&self, _path: &str) -> String {
        "test".to_string()
    }
}

fn request_with(uri: &str, user_agent: &str) -> Request {
    let mut req = Request::builder()
        .uri(uri)
        .body(Body::empty())
        .expect("build request");
    req.headers_mut()
        .insert("user-agent", HeaderValue::from_str(user_agent).unwrap());
    req
}

#[test]
fn from_request_extracts_utm_and_landing_page() {
    let request = request_with(
        "https://example.com/guide?utm_source=news&utm_campaign=launch",
        "Mozilla/5.0 (Windows NT 10.0) Chrome/120.0",
    );
    let routing = HtmlRouting;
    let analytics = SessionAnalytics::from_request(&request, None, Some(&routing));

    assert_eq!(analytics.utm_source.as_deref(), Some("news"));
    assert_eq!(analytics.utm_campaign.as_deref(), Some("launch"));
    assert_eq!(analytics.landing_page.as_deref(), Some("/guide"));
    assert!(
        analytics
            .entry_url
            .as_deref()
            .is_some_and(|u| u.contains("/guide"))
    );
    assert_eq!(
        analytics.user_agent.as_deref().unwrap(),
        "Mozilla/5.0 (Windows NT 10.0) Chrome/120.0"
    );
}

#[test]
fn from_request_without_routing_leaves_landing_page_unset() {
    let request = request_with("https://example.com/x", "Mozilla/5.0 Chrome/120.0");
    let analytics = SessionAnalytics::from_request(&request, None, None);

    assert!(analytics.landing_page.is_none());
    assert!(analytics.entry_url.is_none());
}

#[test]
fn should_skip_tracking_is_false_for_ai_crawler() {
    let request = request_with("https://example.com/", "GPTBot/1.0");
    let analytics = SessionAnalytics::from_request(&request, None, None);

    assert!(analytics.is_ai_crawler());
    // The AI-crawler exemption short-circuits every skip heuristic.
    assert!(!analytics.should_skip_tracking());
}

#[test]
fn should_skip_tracking_is_true_for_plain_bot() {
    let request = request_with("https://example.com/", "Googlebot/2.1");
    let analytics = SessionAnalytics::from_request(&request, None, None);

    assert!(!analytics.is_ai_crawler());
    assert!(analytics.should_skip_tracking());
}
