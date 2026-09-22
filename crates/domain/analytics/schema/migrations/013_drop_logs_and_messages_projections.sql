-- The `logs` and `ai_request_messages` projections are gone. They were the
-- bulk of every baseline rebuild and of the capture traffic, and had two
-- readers: agent top errors now group on analytics_report_agent_tasks
-- .error_message, and gateway session message counts read the new
-- `message_count` column on analytics_report_ai_requests (maintained on
-- ai_requests by a statement trigger over ai_request_messages).
DROP TABLE IF EXISTS analytics_report_logs;
DROP TABLE IF EXISTS analytics_report_ai_request_messages;
ALTER TABLE analytics_report_ai_requests ADD COLUMN IF NOT EXISTS message_count INTEGER NOT NULL DEFAULT 0;

-- Queued facts for a source the projector no longer knows would raise
-- "Unknown reporting source" on delivery; facts for ai_requests captured under
-- the previous column contract gain the new column so they still validate.
-- The outbox belongs to the events extension, which may not have migrated
-- yet on some upgrade paths, so both repairs check for the column first.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'event_outbox' AND column_name = 'fact'
    ) THEN
        DELETE FROM event_outbox
        WHERE consumer = 'analytics_reporting' AND processed_at IS NULL
          AND fact->'data'->>'source' IN ('logs', 'ai_request_messages');
        UPDATE event_outbox
        SET fact = jsonb_set(fact, '{data,row,message_count}', '0'::jsonb)
        WHERE consumer = 'analytics_reporting' AND processed_at IS NULL
          AND fact->'data'->>'source' = 'ai_requests'
          AND jsonb_typeof(fact->'data'->'row') = 'object'
          AND NOT (fact->'data'->'row' ? 'message_count');
    END IF;
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'ai_requests' AND column_name = 'message_count'
    ) THEN
        UPDATE analytics_report_ai_requests r SET message_count = a.message_count
        FROM ai_requests a WHERE a.id = r.id AND r.message_count <> a.message_count;
    END IF;
END
$$;
