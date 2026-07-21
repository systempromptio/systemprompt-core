//! One request, one classification.
//!
//! The verdicts the pipeline routes and records on — `is_bot`,
//! `is_ai_crawler`, `skip_tracking` — are decided once, at extraction. These
//! tests pin the relationships between them so a caller can never observe two
//! different answers for the same request.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_analytics::SessionAnalyticsBuilder;

fn with_user_agent(ua: &str) -> HeaderMap {
    let mut headers = HeaderMap::new();
    headers.insert("user-agent", HeaderValue::from_str(ua).unwrap());
    headers
}

#[test]
fn ai_crawlers_are_neither_bots_nor_suppressed() {
    for ua in [
        "Mozilla/5.0 ChatGPT-User/1.0",
        "Mozilla/5.0 (compatible; ClaudeBot/1.0)",
        "Mozilla/5.0 (compatible; Google-NotebookLM)",
        "Mozilla/5.0 GPTBot/1.0",
        "PerplexityBot/2",
        "anthropic-ai/1",
    ] {
        let headers = with_user_agent(ua);
        let analytics = SessionAnalyticsBuilder::new(&headers).build();
        assert!(analytics.is_ai_crawler, "{ua} should be an AI crawler");
        assert!(!analytics.is_bot, "{ua} must not also be a bot");
        assert!(!analytics.skip_tracking, "{ua} must stay tracked");
    }
}

#[test]
fn malformed_user_agents_are_bots() {
    for ua in ["{USER_AGENT}", "-", "{some_template_var}", "null"] {
        let headers = with_user_agent(ua);
        let analytics = SessionAnalyticsBuilder::new(&headers).build();
        assert!(analytics.is_bot, "{ua} should classify as a bot");
    }
}

#[test]
fn a_bot_user_agent_is_both_recorded_and_suppressed() {
    let headers = with_user_agent("Googlebot/2.1");
    let analytics = SessionAnalyticsBuilder::new(&headers).build();

    assert!(analytics.is_bot);
    assert!(
        analytics.skip_tracking,
        "the verdict written to the session row and the one the middleware routes on must agree"
    );
}

#[test]
fn a_browser_is_neither_bot_nor_suppressed() {
    let headers = with_user_agent(
        "Mozilla/5.0 (Macintosh; Intel Mac OS X 10_15_7) AppleWebKit/537.36 (KHTML, like Gecko) Chrome/120.0 Safari/537.36",
    );
    let analytics = SessionAnalyticsBuilder::new(&headers).build();

    assert!(!analytics.is_bot);
    assert!(!analytics.is_ai_crawler);
    assert!(!analytics.skip_tracking);
}

#[test]
fn an_absent_user_agent_is_a_bot() {
    let headers = HeaderMap::new();
    let analytics = SessionAnalyticsBuilder::new(&headers).build();

    assert!(analytics.is_bot);
    assert!(analytics.skip_tracking);
}
