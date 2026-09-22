-- Until 0.59.0 the gateway settled every request without ever touching
-- user_sessions, so ai_request_count, total_tokens_used and
-- total_ai_cost_microdollars sat at their defaults on every session that had
-- ever paid for inference. settle() now increments them; this recovers the
-- history from ai_requests, which is the ledger those counters summarise.
--
-- This lives in the ai crate, not users: it reads ai_requests, and only the
-- ai crate's migrations are guaranteed to run after that table exists.
-- System traffic is excluded to match the settlement path, which accounts
-- nothing against a session for user_id 'system'.
UPDATE user_sessions s
SET ai_request_count = r.request_count,
    total_tokens_used = r.tokens_used,
    total_ai_cost_microdollars = r.cost_microdollars,
    last_activity_at = GREATEST(s.last_activity_at, r.last_request_at)
FROM (
    SELECT session_id,
           count(*)::INTEGER AS request_count,
           COALESCE(sum(tokens_used), 0)::INTEGER AS tokens_used,
           COALESCE(sum(cost_microdollars), 0)::BIGINT AS cost_microdollars,
           max(created_at) AS last_request_at
    FROM ai_requests
    WHERE session_id IS NOT NULL
      AND user_id <> 'system'
    GROUP BY session_id
) r
WHERE s.session_id = r.session_id
  AND (
      COALESCE(s.ai_request_count, 0) <> r.request_count
      OR COALESCE(s.total_tokens_used, 0) <> r.tokens_used
      OR COALESCE(s.total_ai_cost_microdollars, 0) <> r.cost_microdollars
  );
