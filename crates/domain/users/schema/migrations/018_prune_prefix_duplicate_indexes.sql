-- Drop every index on this crate's tables whose column list is a strict
-- prefix of another index on the same table. `user_sessions` is written on
-- every request, so each prefix index is a write it never repays with a read
-- its superset could not serve.
--
-- Each one names the index that already covers it. None is UNIQUE and none
-- backs a constraint. A leading column indexed ASC here and DESC in the
-- superset is still covered: btree scans a prefix in either direction.

-- covered by users_email_key (same column)
DROP INDEX IF EXISTS idx_users_email;

-- covered by idx_sessions_fingerprint_activity
DROP INDEX IF EXISTS idx_sessions_fingerprint;
-- covered by idx_sessions_started_bot (started_at DESC, is_bot)
DROP INDEX IF EXISTS idx_sessions_started_at;
-- covered by idx_user_sessions_client_activity and ..._client_cost
DROP INDEX IF EXISTS idx_user_sessions_client_id;
-- covered by idx_sessions_bot_time (is_bot, started_at)
DROP INDEX IF EXISTS idx_user_sessions_is_bot;
-- covered by idx_sessions_landing (landing_page, is_bot)
DROP INDEX IF EXISTS idx_user_sessions_landing_page;
