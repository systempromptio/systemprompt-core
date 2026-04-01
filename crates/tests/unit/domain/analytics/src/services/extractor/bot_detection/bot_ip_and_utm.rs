//! Tests for bot IP ranges, UTM parameters, and basic advanced bot detection.

use axum::http::{HeaderMap, HeaderValue, Uri};
use systemprompt_analytics::SessionAnalytics;

fn create_headers_with_user_agent(ua: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_str(ua).unwrap());
    headers
}

fn create_headers_with_ip(ip: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("x-forwarded-for", HeaderValue::from_str(ip).unwrap());
    headers
}

fn create_full_headers() -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert(
        "user-agent",
        HeaderValue::from_static("Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0.0.0"),
    );
    headers.insert("x-forwarded-for", HeaderValue::from_static("192.168.1.1"));
    headers.insert("x-fingerprint", HeaderValue::from_static("fp_abc123"));
    headers.insert("accept-language", HeaderValue::from_static("en-US,en;q=0.9"));
    headers.insert(
        "referer",
        HeaderValue::from_static("https://google.com/search?q=test"),
    );
    headers
}

#[test]
fn is_bot_ip_returns_true_for_microsoft_157_ip() {
    let headers = create_headers_with_ip("157.55.39.1");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot_ip());
}

#[test]
fn is_bot_ip_returns_true_for_microsoft_207_ip() {
    let headers = create_headers_with_ip("207.46.13.1");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot_ip());
}

#[test]
fn is_bot_ip_returns_true_for_facebook_69_ip() {
    let headers = create_headers_with_ip("69.171.250.1");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot_ip());
}

#[test]
fn is_bot_ip_returns_true_for_facebook_173_ip() {
    let headers = create_headers_with_ip("173.252.88.1");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot_ip());
}

#[test]
fn is_bot_ip_returns_true_for_facebook_31_ip() {
    let headers = create_headers_with_ip("31.13.24.1");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot_ip());
}

#[test]
fn from_headers_and_uri_extracts_utm_source() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/page?utm_source=google".parse().unwrap();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, None);
    assert_eq!(analytics.utm_source, Some("google".to_string()));
}

#[test]
fn from_headers_and_uri_extracts_utm_medium() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/page?utm_medium=cpc".parse().unwrap();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, None);
    assert_eq!(analytics.utm_medium, Some("cpc".to_string()));
}

#[test]
fn from_headers_and_uri_extracts_utm_campaign() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/page?utm_campaign=summer_sale"
        .parse()
        .unwrap();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, None);
    assert_eq!(analytics.utm_campaign, Some("summer_sale".to_string()));
}

#[test]
fn from_headers_and_uri_extracts_all_utm_params() {
    let headers = create_full_headers();
    let uri: Uri = "https://example.com/?utm_source=google&utm_medium=cpc&utm_campaign=test"
        .parse()
        .unwrap();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, Some(&uri), None, None);
    assert_eq!(analytics.utm_source, Some("google".to_string()));
    assert_eq!(analytics.utm_medium, Some("cpc".to_string()));
    assert_eq!(analytics.utm_campaign, Some("test".to_string()));
}

#[test]
fn from_headers_and_uri_without_uri() {
    let headers = create_full_headers();
    let analytics = SessionAnalytics::from_headers_and_uri(&headers, None, None, None);
    assert!(analytics.utm_source.is_none());
    assert!(analytics.entry_url.is_none());
    assert!(analytics.landing_page.is_none());
}

#[test]
fn is_bot_detects_gptbot() {
    let headers = create_headers_with_user_agent("GPTBot/1.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_claudeweb() {
    let headers = create_headers_with_user_agent("Claude-Web/1.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_facebookexternalhit() {
    let headers = create_headers_with_user_agent("facebookexternalhit/1.1");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_yandexbot() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; YandexBot/3.0)");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_baiduspider() {
    let headers = create_headers_with_user_agent("Baiduspider/2.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_slurp() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; Yahoo! Slurp)");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_wget() {
    let headers = create_headers_with_user_agent("Wget/1.21");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_puppeteer() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 Puppeteer/19.0.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_phantomjs() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 PhantomJS/2.1.1");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_uptimerobot() {
    let headers = create_headers_with_user_agent("UptimeRobot/2.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_semrushbot() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; SemrushBot/7)");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_ahrefsbot() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; AhrefsBot/7.0)");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}
