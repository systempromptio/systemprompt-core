ALTER TABLE event_outbox ADD COLUMN IF NOT EXISTS consumer TEXT;
ALTER TABLE event_outbox ADD COLUMN IF NOT EXISTS fact JSONB;
ALTER TABLE event_outbox ADD COLUMN IF NOT EXISTS processed_at TIMESTAMPTZ;
ALTER TABLE event_outbox ADD COLUMN IF NOT EXISTS deliver_to_origin BOOLEAN NOT NULL DEFAULT FALSE;
ALTER TABLE event_outbox DROP CONSTRAINT IF EXISTS event_outbox_fact_pair;
ALTER TABLE event_outbox ADD CONSTRAINT event_outbox_fact_pair CHECK (
    (consumer IS NULL AND fact IS NULL AND processed_at IS NULL)
    OR (consumer IS NOT NULL AND length(consumer) > 0 AND fact IS NOT NULL)
);
CREATE INDEX IF NOT EXISTS idx_event_outbox_pending
    ON event_outbox(consumer, created_at, id)
    WHERE consumer IS NOT NULL AND processed_at IS NULL;
