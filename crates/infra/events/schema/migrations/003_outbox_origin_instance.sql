ALTER TABLE event_outbox ADD COLUMN IF NOT EXISTS origin_instance_id TEXT NOT NULL DEFAULT 'unknown';
ALTER TABLE event_outbox ALTER COLUMN origin_instance_id DROP DEFAULT;
