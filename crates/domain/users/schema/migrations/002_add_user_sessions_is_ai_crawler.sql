
ALTER TABLE user_sessions ADD COLUMN IF NOT EXISTS is_ai_crawler BOOLEAN NOT NULL DEFAULT false;
CREATE INDEX IF NOT EXISTS idx_user_sessions_is_ai_crawler ON user_sessions(is_ai_crawler) WHERE is_ai_crawler = true;
UPDATE user_sessions
SET is_ai_crawler = true,
    is_bot = false
WHERE is_ai_crawler = false
  AND user_agent IS NOT NULL
  AND (
        user_agent ILIKE '%NotebookLM%'
     OR user_agent ILIKE '%Gemini-Deep-Research%'
     OR user_agent ILIKE '%Grammarly%'
     OR user_agent ILIKE '%ChatGPT-User%'
     OR user_agent ILIKE '%OAI-SearchBot%'
     OR user_agent ILIKE '%GPTBot%'
     OR user_agent ILIKE '%PerplexityBot%'
     OR user_agent ILIKE '%Perplexity-User%'
     OR user_agent ILIKE '%ClaudeBot%'
     OR user_agent ILIKE '%Claude-User%'
     OR user_agent ILIKE '%Claude-Web%'
     OR user_agent ILIKE '%anthropic-ai%'
     OR user_agent ILIKE '%Applebot-Extended%'
     OR user_agent ILIKE '%CCBot%'
     OR user_agent ILIKE '%Bytespider%'
     OR user_agent ILIKE '%Amazonbot%'
     OR user_agent ILIKE '%YouBot%'
     OR user_agent ILIKE '%Diffbot%'
     OR user_agent ILIKE '%cohere-ai%'
  );
