-- Drop every index on this crate's tables whose column list is a strict
-- prefix of another index on the same table. A prefix index answers no query
-- its superset cannot, so it is pure write amplification: `ai_requests`
-- carried 22 indexes for 2.5 MB of heap and `ai_request_messages` held three
-- indexes on the same (request_id, sequence_number) key.
--
-- Each one names the index that already covers it. None is UNIQUE and none
-- backs a constraint, so dropping them changes no guarantee.

-- covered by ai_quota_buckets_subject_key (subject_kind, subject_id, window_seconds, window_start)
DROP INDEX IF EXISTS idx_ai_quota_buckets_subject;

-- covered by idx_ai_request_messages_sequence, itself covered by
-- ai_request_messages_request_id_sequence_number_key (same columns)
DROP INDEX IF EXISTS idx_ai_request_messages_request_id;
DROP INDEX IF EXISTS idx_ai_request_messages_sequence;

-- covered by ai_requests_request_id_key (same column)
DROP INDEX IF EXISTS idx_ai_requests_request_id;
-- covered by idx_ai_requests_provider_status
DROP INDEX IF EXISTS idx_ai_requests_provider;
-- covered by idx_ai_requests_session_created
DROP INDEX IF EXISTS idx_ai_requests_session_id;
-- covered by idx_ai_requests_user_created and idx_ai_requests_user_model
DROP INDEX IF EXISTS idx_ai_requests_user_id;

-- covered by ai_request_tool_calls_request_id_sequence_number_key
DROP INDEX IF EXISTS idx_ai_request_tool_calls_request_id;
