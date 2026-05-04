//! Static keyword list and matcher for known AI training / search crawlers.

/// Lowercase substrings identifying AI-platform crawlers and agents.
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

/// Returns `true` when `user_agent` contains any keyword from
/// [`AI_CRAWLER_KEYWORDS`] (case-insensitive).
pub fn matches_ai_crawler(user_agent: &str) -> bool {
    let ua_lower = user_agent.to_lowercase();
    AI_CRAWLER_KEYWORDS
        .iter()
        .any(|keyword| ua_lower.contains(keyword))
}
