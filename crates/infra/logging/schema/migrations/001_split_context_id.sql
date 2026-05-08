-- Split conflated context_id into typed namespaces in logs and analytics_events.
-- See crates/domain/ai/schema/migrations/002_split_context_id.sql for the rule.

ALTER TABLE logs
    ADD COLUMN IF NOT EXISTS gateway_conversation_id VARCHAR(255),
    ADD COLUMN IF NOT EXISTS provider_request_id VARCHAR(255);

UPDATE logs
SET gateway_conversation_id = context_id,
    context_id = NULL
WHERE gateway_conversation_id IS NULL
  AND context_id ~ '^ctx_[0-9a-f]{16}$';

UPDATE logs
SET provider_request_id = context_id,
    context_id = NULL
WHERE provider_request_id IS NULL
  AND context_id IS NOT NULL
  AND context_id !~ '^ctx_[0-9a-f]{16}$'
  AND context_id !~ '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$';

CREATE INDEX IF NOT EXISTS idx_logs_gateway_conversation_id
    ON logs(gateway_conversation_id);
CREATE INDEX IF NOT EXISTS idx_logs_provider_request_id
    ON logs(provider_request_id);

ALTER TABLE analytics_events
    ADD COLUMN IF NOT EXISTS gateway_conversation_id VARCHAR(255),
    ADD COLUMN IF NOT EXISTS provider_request_id VARCHAR(255);

UPDATE analytics_events
SET gateway_conversation_id = context_id,
    context_id = NULL
WHERE gateway_conversation_id IS NULL
  AND context_id ~ '^ctx_[0-9a-f]{16}$';

UPDATE analytics_events
SET provider_request_id = context_id,
    context_id = NULL
WHERE provider_request_id IS NULL
  AND context_id IS NOT NULL
  AND context_id !~ '^ctx_[0-9a-f]{16}$'
  AND context_id !~ '^[0-9a-fA-F]{8}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{4}-[0-9a-fA-F]{12}$';

CREATE INDEX IF NOT EXISTS idx_analytics_events_gateway_conversation_id
    ON analytics_events(gateway_conversation_id);
CREATE INDEX IF NOT EXISTS idx_analytics_events_provider_request_id
    ON analytics_events(provider_request_id);
