-- Phase 2c: agents migrate from Postgres to disk.
-- Runtime tables (services, agent_tasks, task_messages, context_agents) keep
-- agent_name as opaque text without an FK to agents, so dropping is safe.
DROP TABLE IF EXISTS agents CASCADE;
