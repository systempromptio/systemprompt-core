-- Session bot type: the canonical user-agent bot taxonomy, stored on the
-- session row instead of re-derived by every reader (store, don't compute).
--
--   user_sessions.bot_type  the taxonomy label of a session with any bot
--                           classification (`is_bot`, `is_ai_crawler`,
--                           `is_scanner`, `is_behavioral_bot`); NULL on a
--                           human session, and on a bot session recorded
--                           before the column existed until the
--                           `session_bot_type_rollup` job stamps it
--
-- `session_bot_type` is the one rule. Statement triggers stamp it when a
-- session is inserted and when a classification flag changes (the
-- behavioural and scanner detectors flip flags on existing rows); its own
-- write changes no flag, so the update trigger finds no key and stops.
-- `refresh_session_bot_type(keys)` applies the same rule to queued session
-- ids; migration 023 queues the bot sessions that predate the column.
CREATE OR REPLACE FUNCTION session_bot_type(
    ua TEXT, bot BOOLEAN, ai_crawler BOOLEAN, scanner BOOLEAN, behavioral BOOLEAN)
RETURNS TEXT LANGUAGE sql IMMUTABLE AS $$
SELECT CASE
    WHEN NOT (COALESCE(bot, false) OR COALESCE(ai_crawler, false)
              OR COALESCE(scanner, false) OR COALESCE(behavioral, false)) THEN NULL
    WHEN ua ILIKE '%googlebot%' OR ua ILIKE '%google-inspectiontool%' OR ua ILIKE '%adsbot-google%' THEN 'Google'
    WHEN ua ILIKE '%bingbot%' OR ua ILIKE '%bingpreview%' OR ua ILIKE '%msnbot%' THEN 'Bing'
    WHEN ua ILIKE '%chatgpt%' OR ua ILIKE '%gptbot%' THEN 'OpenAI'
    WHEN ua ILIKE '%claude%' OR ua ILIKE '%anthropic%' THEN 'Anthropic'
    WHEN ua ILIKE '%perplexity%' THEN 'Perplexity'
    WHEN ua ILIKE '%baiduspider%' THEN 'Baidu'
    WHEN ua ILIKE '%yandexbot%' THEN 'Yandex'
    WHEN ua ILIKE '%facebookexternalhit%' OR ua ILIKE '%facebot%' OR ua ILIKE '%meta-externalagent%' THEN 'Meta'
    WHEN ua ILIKE '%twitterbot%' THEN 'Twitter/X'
    WHEN ua ILIKE '%linkedinbot%' THEN 'LinkedIn'
    WHEN ua ILIKE '%semrushbot%' OR ua ILIKE '%ahrefsbot%' OR ua ILIKE '%mj12bot%' OR ua ILIKE '%dotbot%' THEN 'SEO Crawlers'
    WHEN ua ILIKE '%bytespider%' THEN 'ByteDance'
    WHEN ua ILIKE '%amazonbot%' OR ua ILIKE '%applebot%' THEN 'Tech Giants'
    WHEN ua ILIKE '%python%' OR ua ILIKE '%scrapy%' OR ua ILIKE '%httpx%' THEN 'Python Scrapers'
    WHEN ua ILIKE '%curl%' OR ua ILIKE '%wget%' OR ua ILIKE '%node-fetch%' OR ua ILIKE '%axios%' THEN 'CLI/HTTP Tools'
    WHEN ua ILIKE '%headless%' OR ua ILIKE '%phantom%' OR ua ILIKE '%selenium%' OR ua ILIKE '%puppeteer%' THEN 'Headless Browsers'
    WHEN ua ILIKE '%uptimerobot%' OR ua ILIKE '%pingdom%' OR ua ILIKE '%statuscake%' OR ua ILIKE '%lighthouse%' THEN 'Monitoring'
    WHEN COALESCE(ai_crawler, false) THEN 'AI Crawler'
    WHEN COALESCE(behavioral, false) THEN 'Behavioral Bot'
    WHEN COALESCE(scanner, false) THEN 'Scanner'
    ELSE 'Other'
END
$$;

CREATE OR REPLACE FUNCTION refresh_session_bot_type(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE written BIGINT;
BEGIN
    UPDATE user_sessions s
    SET bot_type = session_bot_type(s.user_agent, s.is_bot, s.is_ai_crawler, s.is_scanner, s.is_behavioral_bot)
    WHERE s.session_id = ANY(keys)
      AND s.bot_type IS DISTINCT FROM
          session_bot_type(s.user_agent, s.is_bot, s.is_ai_crawler, s.is_scanner, s.is_behavioral_bot);
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION stamp_session_bot_type_inserted()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE keys TEXT[];
BEGIN
    keys := ARRAY(SELECT n.session_id FROM new_rows n
                  WHERE n.is_bot OR n.is_ai_crawler OR n.is_scanner OR n.is_behavioral_bot);
    IF cardinality(keys) > 0 THEN PERFORM refresh_session_bot_type(keys); END IF;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION stamp_session_bot_type_updated()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE keys TEXT[];
BEGIN
    keys := ARRAY(SELECT n.session_id FROM new_rows n
                  JOIN old_rows o ON o.session_id = n.session_id
                  WHERE (n.is_bot, n.is_ai_crawler, n.is_scanner, n.is_behavioral_bot, n.user_agent)
                        IS DISTINCT FROM (o.is_bot, o.is_ai_crawler, o.is_scanner, o.is_behavioral_bot, o.user_agent));
    IF cardinality(keys) > 0 THEN PERFORM refresh_session_bot_type(keys); END IF;
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS session_bot_type_insert ON user_sessions;
CREATE TRIGGER session_bot_type_insert
    AFTER INSERT ON user_sessions
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION stamp_session_bot_type_inserted();

DROP TRIGGER IF EXISTS session_bot_type_update ON user_sessions;
CREATE TRIGGER session_bot_type_update
    AFTER UPDATE ON user_sessions
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION stamp_session_bot_type_updated();
