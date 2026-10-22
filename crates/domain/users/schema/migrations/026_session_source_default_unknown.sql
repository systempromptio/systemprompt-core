-- `user_sessions.session_source` defaults to the value `SessionSource` names
-- as its own default, and the human-traffic predicate is one SQL function
-- behind the canonical traffic views.
ALTER TABLE user_sessions ALTER COLUMN session_source SET DEFAULT 'unknown';

CREATE OR REPLACE FUNCTION is_human_traffic(
    is_bot BOOLEAN,
    is_ai_crawler BOOLEAN,
    is_scanner BOOLEAN,
    is_behavioral_bot BOOLEAN
) RETURNS BOOLEAN
LANGUAGE sql IMMUTABLE PARALLEL SAFE
AS $$
    SELECT NOT (is_bot OR is_ai_crawler OR is_scanner OR is_behavioral_bot)
$$;

COMMENT ON FUNCTION is_human_traffic(BOOLEAN, BOOLEAN, BOOLEAN, BOOLEAN) IS 'Canonical human-traffic predicate: true when no bot classification is set. v_clean_traffic, v_engaged_traffic and every Rust FILTER call this instead of restating the four flags';

CREATE OR REPLACE VIEW v_clean_traffic AS
SELECT * FROM user_sessions
WHERE is_human_traffic(is_bot, is_ai_crawler, is_scanner, is_behavioral_bot);

CREATE OR REPLACE VIEW v_engaged_traffic AS
SELECT * FROM user_sessions
WHERE is_human_traffic(is_bot, is_ai_crawler, is_scanner, is_behavioral_bot)
  AND landing_page IS NOT NULL
  AND request_count > 0;

COMMENT ON COLUMN user_sessions.session_source IS 'Origin of the session: web (browser), api (programmatic), cli (command line), oauth (token endpoint), mcp, bridge; unknown when the client could not be classified';
