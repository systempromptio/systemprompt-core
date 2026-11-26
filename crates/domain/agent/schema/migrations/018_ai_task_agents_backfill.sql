-- The agent of each task is kept in the ai member's `ai_task_agents`
-- (schema/ai_task_agents_sync.sql), which the AI and tool-usage rollups read
-- their `agent_name` dimension from. The declarative schema creates the table
-- and the triggers; the tasks that predate them are queued here, keys only,
-- by the hour they were created, for `refresh_ai_task_agents`, which the
-- `backfill_rollups` job drains.
INSERT INTO projection_backlog (projection, key)
SELECT 'ai_task_agents',
       to_char(date_trunc('hour', created_at, 'UTC') AT TIME ZONE 'UTC', 'YYYY-MM-DD"T"HH24":00:00Z"')
FROM agent_tasks
WHERE agent_name IS NOT NULL
ON CONFLICT DO NOTHING;
