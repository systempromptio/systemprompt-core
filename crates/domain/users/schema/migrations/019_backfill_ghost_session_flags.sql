-- Clear the `ghost_session` bot flag from every session the ghost predicate
-- should never have reached.
--
-- `ghost_session_cleanup` marked any session with no page views as a
-- behavioural bot. A bridge, API or MCP session has no page views by
-- construction — it is a gateway credential holder, not a browser — so every
-- one of them was flagged: on the reference instance 1,068 bridge sessions
-- alone, which is what made the traffic dashboards read as bot traffic. The
-- job's predicate now restricts itself to `session_source = 'web'`; this is
-- the backfill for the rows it already wrote.
--
-- Only the `ghost_session` reason is cleared. A session flagged by the
-- behavioural analyser for any other reason keeps its flag.
UPDATE user_sessions
SET is_behavioral_bot = false,
    behavioral_bot_reason = NULL
WHERE behavioral_bot_reason = 'ghost_session'
  AND session_source <> 'web';
