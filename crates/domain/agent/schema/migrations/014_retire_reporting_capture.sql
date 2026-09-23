-- The reporting projection is retired: its capture triggers and source views
-- on this extension's tables go, with the privacy routine that locked them.
DROP TRIGGER IF EXISTS reporting_capture_insert ON agent_tasks;
DROP TRIGGER IF EXISTS reporting_capture_update ON agent_tasks;
DROP TRIGGER IF EXISTS reporting_capture_delete ON agent_tasks;
DROP TRIGGER IF EXISTS reporting_capture ON agent_tasks;
DROP VIEW IF EXISTS reporting_source_agent_tasks;
DROP TRIGGER IF EXISTS reporting_capture_insert ON task_messages;
DROP TRIGGER IF EXISTS reporting_capture_update ON task_messages;
DROP TRIGGER IF EXISTS reporting_capture_delete ON task_messages;
DROP TRIGGER IF EXISTS reporting_capture ON task_messages;
DROP VIEW IF EXISTS reporting_source_task_messages;
DROP TRIGGER IF EXISTS reporting_capture_insert ON user_contexts;
DROP TRIGGER IF EXISTS reporting_capture_update ON user_contexts;
DROP TRIGGER IF EXISTS reporting_capture_delete ON user_contexts;
DROP TRIGGER IF EXISTS reporting_capture ON user_contexts;
DROP VIEW IF EXISTS reporting_source_user_contexts;
DROP FUNCTION IF EXISTS public.lock_agent_reporting_sources();
DROP FUNCTION IF EXISTS public.reporting_task_is_retained(TEXT, TIMESTAMPTZ);
