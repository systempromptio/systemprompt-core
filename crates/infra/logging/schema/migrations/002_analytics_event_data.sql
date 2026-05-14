-- Bring legacy analytics_events tables in line with the declarative schema,
-- which now defines event_data as a JSONB column. Fresh installs pick the
-- column up via schema/analytics.sql; existing databases need this migration
-- so the GIN index in the schema has a column to reference.
ALTER TABLE analytics_events
    ADD COLUMN IF NOT EXISTS event_data JSONB DEFAULT '{}';
