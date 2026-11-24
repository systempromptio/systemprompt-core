//! Tests for bot keyword detection, malformed UA detection, and bot IP range
//! checks.

use systemprompt_analytics::services::bot_keywords::{
    BOT_IP_PREFIXES, BOT_KEYWORDS, is_malformed_user_agent, matches_bot_ip_range,
    matches_bot_pattern,
};

mod is_malformed_user_agent_tests {
    use super::*;

    #[test]
    fn empty_string_is_malformed() {
        assert!(is_malformed_user_agent(""));
    }

    #[test]
    fn short_string_under_10_chars_is_malformed() {
        assert!(is_malformed_user_agent("Mozilla/5"));
    }

    #[test]
    fn exactly_9_chars_is_malformed() {
        assert!(is_malformed_user_agent("123456789"));
    }

    #[test]
    fn dash_is_malformed() {
        assert!(is_malformed_user_agent("-"));
    }

    #[test]
    fn null_literal_is_malformed() {
        assert!(is_malformed_user_agent("null"));
    }

    #[test]
    fn unknown_literal_is_malformed() {
        assert!(is_malformed_user_agent("unknown"));
    }

    #[test]
    fn template_variable_open_brace_is_malformed() {
        assert!(is_malformed_user_agent("{USER_AGENT_STRING}"));
    }

    #[test]
    fn template_variable_close_brace_is_malformed() {
        assert!(is_malformed_user_agent("some template}"));
    }

    #[test]
    fn valid_chrome_ua_is_not_malformed() {
        assert!(!is_malformed_user_agent(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) Chrome/120.0"
        ));
    }

    #[test]
    fn valid_firefox_ua_is_not_malformed() {
        assert!(!is_malformed_user_agent(
            "Mozilla/5.0 (X11; Linux x86_64; rv:121.0) Gecko/20100101 Firefox/121.0"
        ));
    }

    #[test]
    fn exactly_10_chars_is_not_malformed() {
        assert!(!is_malformed_user_agent("1234567890"));
    }

    #[test]
    fn curl_tool_is_not_malformed_by_length() {
        assert!(!is_malformed_user_agent("curl/7.88.1"));
    }
}

mod matches_bot_pattern_tests {
    use super::*;

    #[test]
    fn googlebot_matches() {
        assert!(matches_bot_pattern("Googlebot/2.1"));
    }

    #[test]
    fn bingbot_matches() {
        assert!(matches_bot_pattern("bingbot/2.0"));
    }

    #[test]
    fn generic_bot_keyword_matches() {
        assert!(matches_bot_pattern(
            "some-bot-agent/1.0 (+http://example.com)"
        ));
    }

    #[test]
    fn crawler_keyword_matches() {
        assert!(matches_bot_pattern("MyCrawler/1.0"));
    }

    #[test]
    fn spider_keyword_matches() {
        assert!(matches_bot_pattern("WebSpider/1.0"));
    }

    #[test]
    fn curl_matches() {
        assert!(matches_bot_pattern("curl/7.88.1"));
    }

    #[test]
    fn wget_matches() {
        assert!(matches_bot_pattern("Wget/1.21"));
    }

    #[test]
    fn python_requests_matches() {
        assert!(matches_bot_pattern("python-requests/2.28.0"));
    }

    #[test]
    fn go_http_client_matches() {
        assert!(matches_bot_pattern("Go-http-client/2.0"));
    }

    #[test]
    fn axios_matches() {
        assert!(matches_bot_pattern("axios/1.0.0"));
    }

    #[test]
    fn selenium_matches() {
        assert!(matches_bot_pattern("Mozilla/5.0 Selenium/4.0"));
    }

    #[test]
    fn headless_matches() {
        assert!(matches_bot_pattern("Mozilla/5.0 HeadlessChrome/120.0"));
    }

    #[test]
    fn puppeteer_matches() {
        assert!(matches_bot_pattern("Mozilla/5.0 Puppeteer/1.0"));
    }

    #[test]
    fn compatible_without_browser_markers_matches() {
        assert!(matches_bot_pattern("Mozilla/5.0 (compatible; SomeBot/1.0)"));
    }

    #[test]
    fn compatible_without_browser_or_bot_keyword_matches() {
        // "MSIE" carries no bot keyword and no known-browser marker, so the
        // sole trigger is the bare `compatible` heuristic branch.
        assert!(matches_bot_pattern(
            "Mozilla/5.0 (compatible; MSIE 10.0; Windows NT 6.1)"
        ));
    }

    #[test]
    fn compatible_with_chrome_is_not_bot() {
        assert!(!matches_bot_pattern(
            "Mozilla/5.0 (compatible; MSIE 10.0; Windows NT 6.1; Chrome/120.0)"
        ));
    }

    #[test]
    fn compatible_with_firefox_is_not_bot() {
        assert!(!matches_bot_pattern(
            "Mozilla/5.0 (compatible; Firefox/121.0)"
        ));
    }

    #[test]
    fn compatible_with_safari_is_not_bot() {
        assert!(!matches_bot_pattern(
            "Mozilla/5.0 (compatible; Safari/605.1)"
        ));
    }

    #[test]
    fn compatible_with_edge_is_not_bot() {
        assert!(!matches_bot_pattern("Mozilla/5.0 (compatible; Edge/120)"));
    }

    #[test]
    fn real_chrome_ua_is_not_bot() {
        assert!(!matches_bot_pattern(
            "Mozilla/5.0 (Windows NT 10.0; Win64; x64) AppleWebKit/537.36 Chrome/120.0 Safari/537.36"
        ));
    }

    #[test]
    fn real_firefox_ua_is_not_bot() {
        assert!(!matches_bot_pattern(
            "Mozilla/5.0 (Macintosh; Intel Mac OS X 14.0; rv:121.0) Gecko/20100101 Firefox/121.0"
        ));
    }

    #[test]
    fn empty_ua_matches_as_bot_via_malformed() {
        assert!(matches_bot_pattern(""));
    }

    #[test]
    fn short_ua_matches_as_bot_via_malformed() {
        assert!(matches_bot_pattern("abc"));
    }

    #[test]
    fn all_bot_keywords_match() {
        for keyword in BOT_KEYWORDS {
            let ua = format!("SomeAgent/{keyword}/1.0");
            assert!(
                matches_bot_pattern(&ua),
                "keyword {keyword} should match as bot"
            );
        }
    }

    #[test]
    fn semrushbot_matches() {
        assert!(matches_bot_pattern("SemrushBot/7"));
    }

    #[test]
    fn ahrefsbot_matches() {
        assert!(matches_bot_pattern("AhrefsBot/7.0"));
    }

    #[test]
    fn uptimerobot_matches() {
        assert!(matches_bot_pattern("UptimeRobot/2.0"));
    }

    #[test]
    fn scrapy_matches() {
        assert!(matches_bot_pattern("Scrapy/2.11.0"));
    }

    #[test]
    fn lighthouse_matches() {
        assert!(matches_bot_pattern("Mozilla/5.0 lighthouse/1.0"));
    }
}

mod matches_bot_ip_range_tests {
    use super::*;

    #[test]
    fn google_crawler_range_matches() {
        assert!(matches_bot_ip_range("66.249.64.1"));
    }

    #[test]
    fn google_crawler_range_66_249_matches() {
        assert!(matches_bot_ip_range("66.249.90.200"));
    }

    #[test]
    fn bing_range_40_77_matches() {
        assert!(matches_bot_ip_range("40.77.10.1"));
    }

    #[test]
    fn bing_range_157_55_matches() {
        assert!(matches_bot_ip_range("157.55.100.50"));
    }

    #[test]
    fn bing_range_207_46_matches() {
        assert!(matches_bot_ip_range("207.46.1.1"));
    }

    #[test]
    fn facebook_range_69_171_matches() {
        assert!(matches_bot_ip_range("69.171.1.1"));
    }

    #[test]
    fn facebook_range_173_252_matches() {
        assert!(matches_bot_ip_range("173.252.100.100"));
    }

    #[test]
    fn facebook_range_31_13_matches() {
        assert!(matches_bot_ip_range("31.13.1.1"));
    }

    #[test]
    fn residential_ip_does_not_match() {
        assert!(!matches_bot_ip_range("192.168.1.1"));
    }

    #[test]
    fn private_ip_10_does_not_match() {
        assert!(!matches_bot_ip_range("10.0.0.1"));
    }

    #[test]
    fn empty_ip_does_not_match() {
        assert!(!matches_bot_ip_range(""));
    }

    #[test]
    fn all_bot_ip_prefixes_match() {
        for prefix in BOT_IP_PREFIXES {
            let ip = format!("{prefix}1");
            assert!(
                matches_bot_ip_range(&ip),
                "prefix {prefix} should match ip {ip}"
            );
        }
    }

    #[test]
    fn partial_prefix_does_not_match() {
        assert!(!matches_bot_ip_range("66.1.1.1"));
    }
}
