//! Bot detection tests for various user agent strings.

use axum::http::{HeaderMap, HeaderValue};
use systemprompt_analytics::SessionAnalytics;

mod bot_detection_tests {
    use super::*;

    fn create_headers_with_user_agent(ua: &str) -> HeaderMap {
        let mut headers = HeaderMap::new();
        headers.insert("user-agent", HeaderValue::from_str(ua).unwrap());
        headers
    }

    #[test]
    fn ai_crawler_detects_gptbot() {
        let headers = create_headers_with_user_agent("GPTBot/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_ai_crawler());
        assert!(!analytics.is_bot());
    }

    #[test]
    fn ai_crawler_detects_claudeweb() {
        let headers = create_headers_with_user_agent("Claude-Web/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_ai_crawler());
        assert!(!analytics.is_bot());
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
    fn ai_crawler_detects_amazonbot() {
        let headers = create_headers_with_user_agent("Amazonbot/0.1");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_ai_crawler());
        assert!(!analytics.is_bot());
    }

    #[test]
    fn ai_crawler_detects_bytespider() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 Bytespider");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_ai_crawler());
        assert!(!analytics.is_bot());
    }

    #[test]
    fn is_bot_detects_petalbot() {
        let headers = create_headers_with_user_agent("Mozilla/5.0 (compatible; PetalBot)");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_bot());
    }

    #[test]
    fn ai_crawler_detects_perplexitybot() {
        let headers = create_headers_with_user_agent("PerplexityBot/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_ai_crawler());
        assert!(!analytics.is_bot());
    }

    #[test]
    fn ai_crawler_detects_chatgpt_user() {
        let headers = create_headers_with_user_agent("ChatGPT-User/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_ai_crawler());
        assert!(!analytics.is_bot());
    }

    #[test]
    fn ai_crawler_detects_anthropic_ai() {
        let headers = create_headers_with_user_agent("Anthropic-AI/1.0");
        let analytics = SessionAnalytics::from_headers(&headers);

        assert!(analytics.is_ai_crawler());
        assert!(!analytics.is_bot());
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
}
