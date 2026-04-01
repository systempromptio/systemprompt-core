//! Tests for content routing integration with session analytics.

use axum::http::{HeaderMap, HeaderValue, Uri};
use systemprompt_analytics::SessionAnalytics;
use systemprompt_models::ContentRouting;

#[derive(Debug)]
struct MockHtmlRouting;

impl ContentRouting for MockHtmlRouting {
    fn is_html_page(&self, _path: &str) -> bool {
        true
    }

    fn determine_source(&self, _path: &str) -> String {
        "test".to_string()
    }
}

#[derive(Debug)]
struct MockNonHtmlRouting;

impl ContentRouting for MockNonHtmlRouting {
    fn is_html_page(&self, _path: &str) -> bool {
        false
    }

    fn determine_source(&self, _path: &str) -> String {
        "test".to_string()
    }
}

fn create_headers_with_user_agent(ua: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_str(ua).unwrap());
    headers
}

fn create_full_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "user-agent",
        HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0"),
    );
    headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.1.1"));
    headers
}

#[test]
fn html_page_sets_entry_url() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/about".parse().unwrap();
    let routing = MockHtmlRouting;
    let analytics =
        SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));
    let entry_url = analytics.entry_url.expect("expected Some value");
    assert!(entry_url.contains("/about"));
}

#[test]
fn html_page_without_referrer_sets_landing_page() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 Chrome/120");
    let uri: Uri = "https://example.com/landing".parse().unwrap();
    let routing = MockHtmlRouting;
    let analytics =
        SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));
    assert_eq!(analytics.landing_page, Some("/landing".to_string()));
}

#[test]
fn html_page_with_same_site_referrer_sets_landing_page() {
    let mut headers = create_full_headers();
    headers.insert(
        "referer",
        HeaderValue::from_static("https://systemprompt.io/other"),
    );
    let uri: Uri = "https://example.com/page".parse().unwrap();
    let routing = MockHtmlRouting;
    let analytics =
        SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));
    assert_eq!(analytics.landing_page, Some("/page".to_string()));
}

#[test]
fn html_page_with_localhost_referrer_sets_landing_page() {
    let mut headers = create_full_headers();
    headers.insert(
        "referer",
        HeaderValue::from_static("http://localhost:3000/test"),
    );
    let uri: Uri = "https://example.com/page".parse().unwrap();
    let routing = MockHtmlRouting;
    let analytics =
        SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));
    assert_eq!(analytics.landing_page, Some("/page".to_string()));
}

#[test]
fn html_page_with_tyingshoelaces_referrer_sets_landing_page() {
    let mut headers = create_full_headers();
    headers.insert(
        "referer",
        HeaderValue::from_static("https://tyingshoelaces.com/blog"),
    );
    let uri: Uri = "https://example.com/page".parse().unwrap();
    let routing = MockHtmlRouting;
    let analytics =
        SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));
    assert_eq!(analytics.landing_page, Some("/page".to_string()));
}

#[test]
fn html_page_with_external_referrer_still_sets_landing_page() {
    let mut headers = create_full_headers();
    headers.insert(
        "referer",
        HeaderValue::from_static("https://google.com/search"),
    );
    let uri: Uri = "https://example.com/page".parse().unwrap();
    let routing = MockHtmlRouting;
    let analytics =
        SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));
    assert_eq!(analytics.landing_page, Some("/page".to_string()));
}

#[test]
fn non_html_page_no_entry_url() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/api/data".parse().unwrap();
    let routing = MockNonHtmlRouting;
    let analytics =
        SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, Some(&routing));
    assert!(analytics.entry_url.is_none());
    assert!(analytics.landing_page.is_none());
}
