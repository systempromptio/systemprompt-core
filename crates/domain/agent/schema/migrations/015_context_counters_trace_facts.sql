-- Context counters (schema/user_context_counters.sql): the columns are the
-- base schema's text, left NULL ("pending") on the contexts that predate
-- them; the default applies to contexts created from here on. Every
-- existing context is queued for `refresh_user_context_counters`, and every
-- task's trace for `refresh_trace_facts` (database schema trace_facts.sql);
-- the `user_context_counters_rollup` and `trace_facts_rollup` jobs drain them.
ALTER TABLE user_contexts ADD COLUMN IF NOT EXISTS task_count BIGINT;
ALTER TABLE user_contexts ADD COLUMN IF NOT EXISTS message_count BIGINT;
ALTER TABLE user_contexts ADD COLUMN IF NOT EXISTS last_message_at TIMESTAMPTZ;
ALTER TABLE user_contexts ALTER COLUMN task_count SET DEFAULT 0;
ALTER TABLE user_contexts ALTER COLUMN message_count SET DEFAULT 0;

INSERT INTO projection_backlog (projection, key)
SELECT 'user_context_counters', context_id FROM user_contexts WHERE task_count IS NULL
ON CONFLICT DO NOTHING;

INSERT INTO projection_backlog (projection, key)
SELECT 'trace_facts', trace_id FROM agent_tasks WHERE trace_id IS NOT NULL AND trace_id <> ''
ON CONFLICT DO NOTHING;
