//! Tests for AI crawler keyword detection.

use systemprompt_analytics::services::ai_crawler_keywords::{
    AI_CRAWLER_KEYWORDS, matches_ai_crawler,
};

mod matches_ai_crawler_tests {
    use super::*;

    #[test]
    fn gptbot_is_ai_crawler() {
        assert!(matches_ai_crawler("GPTBot/1.0"));
    }

    #[test]
    fn chatgpt_user_is_ai_crawler() {
        assert!(matches_ai_crawler("Mozilla/5.0 ChatGPT-User/1.0"));
    }

    #[test]
    fn oai_searchbot_is_ai_crawler() {
        assert!(matches_ai_crawler("oai-searchbot/1.0"));
    }

    #[test]
    fn claudebot_is_ai_crawler() {
        assert!(matches_ai_crawler("ClaudeBot/1.0 (+https://anthropic.com)"));
    }

    #[test]
    fn claude_user_is_ai_crawler() {
        assert!(matches_ai_crawler("Claude-User/1.0"));
    }

    #[test]
    fn anthropic_ai_is_ai_crawler() {
        assert!(matches_ai_crawler("anthropic-ai/1.0"));
    }

    #[test]
    fn perplexitybot_is_ai_crawler() {
        assert!(matches_ai_crawler("PerplexityBot/1.0"));
    }

    #[test]
    fn perplexity_user_is_ai_crawler() {
        assert!(matches_ai_crawler("Mozilla/5.0 perplexity-user/1.0"));
    }

    #[test]
    fn ccbot_is_ai_crawler() {
        assert!(matches_ai_crawler("CCBot/2.0"));
    }

    #[test]
    fn bytespider_is_ai_crawler() {
        assert!(matches_ai_crawler("Bytespider/1.0"));
    }

    #[test]
    fn amazonbot_is_ai_crawler() {
        assert!(matches_ai_crawler("AmazonBot/1.0"));
    }

    #[test]
    fn cohere_ai_is_ai_crawler() {
        assert!(matches_ai_crawler("cohere-ai/1.0"));
    }

    #[test]
    fn notebooklm_is_ai_crawler() {
        assert!(matches_ai_crawler(
            "Mozilla/5.0 (compatible; Google-NotebookLM)"
        ));
    }

    #[test]
    fn diffbot_is_ai_crawler() {
        assert!(matches_ai_crawler("Diffbot/1.0"));
    }

    #[test]
    fn gemini_deep_research_is_ai_crawler() {
        assert!(matches_ai_crawler("Gemini-Deep-Research/1.0"));
    }

    #[test]
    fn youbot_is_ai_crawler() {
        assert!(matches_ai_crawler("YouBot/1.0"));
    }

    #[test]
    fn normal_chrome_is_not_ai_crawler() {
        assert!(!matches_ai_crawler(
            "Mozilla/5.0 (Windows NT 10.0; Win64) Chrome/120.0"
        ));
    }

    #[test]
    fn googlebot_is_not_ai_crawler() {
        assert!(!matches_ai_crawler("Googlebot/2.1"));
    }

    #[test]
    fn empty_ua_is_not_ai_crawler() {
        assert!(!matches_ai_crawler(""));
    }

    #[test]
    fn case_insensitive_matching() {
        assert!(matches_ai_crawler("GPTBOT/1.0"));
        assert!(matches_ai_crawler("claudebot/1.0"));
    }

    #[test]
    fn all_ai_crawler_keywords_match() {
        for keyword in AI_CRAWLER_KEYWORDS {
            assert!(
                matches_ai_crawler(keyword),
                "keyword {keyword} should be detected as AI crawler"
            );
        }
    }

    #[test]
    fn safari_is_not_ai_crawler() {
        assert!(!matches_ai_crawler(
            "Mozilla/5.0 (iPhone; CPU iPhone OS 17_0) AppleWebKit/605.1 Safari/604.1"
        ));
    }

    #[test]
    fn claude_web_is_ai_crawler() {
        assert!(matches_ai_crawler("claude-web/1.0"));
    }

    #[test]
    fn applebot_extended_is_ai_crawler() {
        assert!(matches_ai_crawler("Applebot-Extended/1.0"));
    }

    #[test]
    fn grammarly_is_ai_crawler() {
        assert!(matches_ai_crawler("Grammarly/1.0 (bot; grammarly.com)"));
    }
}
