-- Migration 008 widened the mcp_tool_executions reporting capture with
-- `source`, `correlation` and `payload_sha256`, keys the reporting contract
-- (analytics reporting_privacy.sql) does not list, so every queued fact
-- raised "Reporting row violates versioned contract" on privacy delivery.
-- The capture itself is declarative (schema/reporting_capture.sql) and is
-- re-established after migrations; this strips the keys from facts already
-- queued. It runs before the events extension's own migrations on some
-- upgrade paths, so it checks for the outbox column instead of assuming it.
DO $$
BEGIN
    IF EXISTS (
        SELECT 1 FROM information_schema.columns
        WHERE table_name = 'event_outbox' AND column_name = 'fact'
    ) THEN
        UPDATE event_outbox
        SET fact = jsonb_set(fact, '{data,row}', (fact->'data'->'row') - 'source' - 'correlation' - 'payload_sha256')
        WHERE channel = 'reporting'
          AND processed_at IS NULL
          AND fact->'data'->>'source' = 'mcp_tool_executions'
          AND jsonb_typeof(fact->'data'->'row') = 'object';
    END IF;
END
$$;
