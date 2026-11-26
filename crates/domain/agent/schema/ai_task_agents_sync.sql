-- The agent of each task, kept current in the ai member's `ai_task_agents`,
-- which the AI and tool-usage rollups read their `agent_name` dimension
-- from. This member owns the writer because it owns `agent_tasks`: statement
-- triggers on it upsert the agent of every inserted or updated task that
-- names one, remove the row of a task whose agent is cleared, and remove the
-- rows of deleted tasks. `refresh_ai_task_agents(keys)` rebuilds the rows of
-- the tasks created in whole hours (keys are `YYYY-MM-DDTHH:00:00Z`);
-- migration 018 queues every hour that predates the triggers and the
-- `backfill_rollups` job drains them.

CREATE OR REPLACE FUNCTION refresh_ai_task_agents(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE
    hours TIMESTAMPTZ[];
    written BIGINT;
BEGIN
    hours := ARRAY(SELECT DISTINCT date_trunc('hour', k::timestamptz, 'UTC') FROM unnest(keys) k);
    INSERT INTO ai_task_agents AS a (task_id, agent_name)
    SELECT t.task_id, t.agent_name
    FROM agent_tasks t
    JOIN unnest(hours) h(b) ON t.created_at >= h.b AND t.created_at < h.b + interval '1 hour'
    WHERE t.agent_name IS NOT NULL
    ORDER BY t.task_id
    ON CONFLICT (task_id) DO UPDATE SET agent_name = EXCLUDED.agent_name
    WHERE a.agent_name IS DISTINCT FROM EXCLUDED.agent_name;
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION ai_task_agents_apply()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    IF TG_OP = 'DELETE' THEN
        DELETE FROM ai_task_agents a USING old_rows o WHERE a.task_id = o.task_id;
        RETURN NULL;
    END IF;
    IF TG_OP = 'UPDATE' THEN
        DELETE FROM ai_task_agents a USING new_rows n
        WHERE a.task_id = n.task_id AND n.agent_name IS NULL;
    END IF;
    INSERT INTO ai_task_agents AS a (task_id, agent_name)
    SELECT n.task_id, n.agent_name FROM new_rows n
    WHERE n.agent_name IS NOT NULL
    ORDER BY n.task_id
    ON CONFLICT (task_id) DO UPDATE SET agent_name = EXCLUDED.agent_name
    WHERE a.agent_name IS DISTINCT FROM EXCLUDED.agent_name;
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS ai_task_agents_insert ON agent_tasks;
CREATE TRIGGER ai_task_agents_insert
    AFTER INSERT ON agent_tasks
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION ai_task_agents_apply();

DROP TRIGGER IF EXISTS ai_task_agents_update ON agent_tasks;
CREATE TRIGGER ai_task_agents_update
    AFTER UPDATE ON agent_tasks
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION ai_task_agents_apply();

DROP TRIGGER IF EXISTS ai_task_agents_delete ON agent_tasks;
CREATE TRIGGER ai_task_agents_delete
    AFTER DELETE ON agent_tasks
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION ai_task_agents_apply();
