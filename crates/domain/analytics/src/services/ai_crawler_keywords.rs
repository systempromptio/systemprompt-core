pub const AI_CRAWLER_KEYWORDS: &[&str] = &[
    "notebooklm",
    "gemini-deep-research",
    "grammarly",
    "chatgpt-user",
    "oai-searchbot",
    "gptbot",
    "perplexitybot",
    "perplexity-user",
    "claudebot",
    "claude-user",
    "claude-web",
    "anthropic-ai",
    "applebot-extended",
    "ccbot",
    "bytespider",
    "amazonbot",
    "youbot",
    "diffbot",
    "cohere-ai",
];

pub fn matches_ai_crawler(user_agent: &str) -> bool {
    let ua_lower = user_agent.to_lowercase();
    AI_CRAWLER_KEYWORDS
        .iter()
        .any(|keyword| ua_lower.contains(keyword))
}
