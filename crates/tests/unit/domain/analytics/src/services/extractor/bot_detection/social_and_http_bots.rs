//! Tests for social media bots, HTTP library bots, and AI bots.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_analytics::SessionAnalytics;

fn create_headers_with_user_agent(ua: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_str(ua).unwrap());
    headers
}

#[test]
fn is_bot_detects_duckduckbot() {
    let headers = create_headers_with_user_agent("DuckDuckBot/1.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_slackbot() {
    let headers = create_headers_with_user_agent("Slackbot 1.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_discordbot() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; Discordbot/2.0)");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_twitterbot() {
    let headers = create_headers_with_user_agent("Twitterbot/1.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_linkedinbot() {
    let headers = create_headers_with_user_agent("LinkedInBot/1.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_webdriver() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 WebDriver");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_scrapy() {
    let headers = create_headers_with_user_agent("Scrapy/2.7.1");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_http_library_okhttp() {
    let headers = create_headers_with_user_agent("okhttp/4.10.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_http_library_axios() {
    let headers = create_headers_with_user_agent("axios/1.2.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_go_http_client() {
    let headers = create_headers_with_user_agent("Go-http-client/1.1");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_node_fetch() {
    let headers = create_headers_with_user_agent("node-fetch/3.0.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_archive_org_bot() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 archive.org_bot");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_applebot() {
    let headers = create_headers_with_user_agent("Applebot/0.1");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_amazonbot() {
    let headers = create_headers_with_user_agent("Amazonbot/0.1");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_bytespider() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 Bytespider");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_petalbot() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; PetalBot)");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_perplexitybot() {
    let headers = create_headers_with_user_agent("PerplexityBot/1.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_chatgpt_user() {
    let headers = create_headers_with_user_agent("ChatGPT-User/1.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_anthropic_ai() {
    let headers = create_headers_with_user_agent("Anthropic-AI/1.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_lighthouse() {
    let headers = create_headers_with_user_agent("Mozilla/5.0 Lighthouse/10.0");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}

#[test]
fn is_bot_detects_pingdom() {
    let headers = create_headers_with_user_agent("Pingdom.com_bot_version_1.4");
    let analytics = SessionAnalytics::from_headers(&headers);
    assert!(analytics.is_bot());
}
