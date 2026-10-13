-- Context counters: how many tasks and messages a context holds and when
-- its last message arrived, kept on the user_contexts row so the context
-- list reads one row per context instead of joining every task and message.
--
--   user_contexts.task_count       tasks in the context
--   user_contexts.message_count    messages on those tasks
--   user_contexts.last_message_at  the newest of those messages
--
-- Inserts are additive and applied in the writer's statement. A delete, or
-- a task or message that moves, recounts the contexts it touched through
-- `refresh_user_context_counters`, which is exact. NULL counters mean the
-- context predates them and is still queued (migration 015); increments
-- leave a NULL alone so a partial count never passes for a total, and the
-- `user_context_counters_rollup` job fills it from the queue. A counter write
-- changes no column a person edits, so it does not bump `updated_at`
-- (the WHEN clause on `update_user_contexts_updated_at`).
CREATE OR REPLACE FUNCTION refresh_user_context_counters(keys TEXT[])
RETURNS BIGINT LANGUAGE plpgsql AS $$
DECLARE written BIGINT;
BEGIN
    WITH counts AS (
        SELECT k.context_id,
               (SELECT COUNT(*) FROM agent_tasks t WHERE t.context_id = k.context_id) AS tasks,
               m.messages, m.last_at
        FROM unnest(keys) AS k(context_id)
        CROSS JOIN LATERAL (
            SELECT COUNT(*) AS messages, MAX(tm.created_at) AS last_at
            FROM agent_tasks t JOIN task_messages tm ON tm.task_id = t.task_id
            WHERE t.context_id = k.context_id) m
    )
    UPDATE user_contexts c
    SET task_count = s.tasks, message_count = s.messages, last_message_at = s.last_at
    FROM counts s
    WHERE c.context_id = s.context_id
      AND (c.task_count, c.message_count, c.last_message_at)
          IS DISTINCT FROM (s.tasks, s.messages, s.last_at);
    GET DIAGNOSTICS written = ROW_COUNT;
    RETURN written;
END $$;

CREATE OR REPLACE FUNCTION user_context_counters_task_inserted()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    UPDATE user_contexts c SET task_count = c.task_count + d.tasks
    FROM (SELECT n.context_id, COUNT(*) AS tasks FROM new_rows n GROUP BY n.context_id) d
    WHERE c.context_id = d.context_id AND c.task_count IS NOT NULL;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION user_context_counters_task_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE keys TEXT[];
BEGIN
    IF TG_OP = 'DELETE' THEN
        keys := ARRAY(SELECT DISTINCT o.context_id FROM old_rows o);
    ELSE
        keys := ARRAY(SELECT n.context_id FROM new_rows n
                      JOIN old_rows o ON o.task_id = n.task_id
                      WHERE n.context_id IS DISTINCT FROM o.context_id
                      UNION
                      SELECT o.context_id FROM new_rows n
                      JOIN old_rows o ON o.task_id = n.task_id
                      WHERE n.context_id IS DISTINCT FROM o.context_id);
    END IF;
    IF cardinality(keys) > 0 THEN PERFORM refresh_user_context_counters(keys); END IF;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION user_context_counters_message_inserted()
RETURNS trigger LANGUAGE plpgsql AS $$
BEGIN
    UPDATE user_contexts c
    SET message_count = c.message_count + d.messages,
        last_message_at = GREATEST(c.last_message_at, d.last_at)
    FROM (SELECT t.context_id, COUNT(*) AS messages, MAX(n.created_at) AS last_at
          FROM new_rows n JOIN agent_tasks t ON t.task_id = n.task_id
          GROUP BY t.context_id) d
    WHERE c.context_id = d.context_id AND c.message_count IS NOT NULL;
    RETURN NULL;
END $$;

CREATE OR REPLACE FUNCTION user_context_counters_message_changed()
RETURNS trigger LANGUAGE plpgsql AS $$
DECLARE keys TEXT[];
BEGIN
    IF TG_OP = 'DELETE' THEN
        keys := ARRAY(SELECT o.context_id FROM old_rows o
                      UNION
                      SELECT t.context_id FROM old_rows o JOIN agent_tasks t ON t.task_id = o.task_id);
    ELSE
        keys := ARRAY(
            WITH moved AS (
                SELECT n.context_id AS new_context, o.context_id AS old_context,
                       n.task_id AS new_task, o.task_id AS old_task
                FROM new_rows n JOIN old_rows o ON o.id = n.id
                WHERE (n.task_id, n.created_at) IS DISTINCT FROM (o.task_id, o.created_at))
            SELECT m.new_context FROM moved m
            UNION SELECT m.old_context FROM moved m
            UNION SELECT t.context_id FROM moved m
                  JOIN agent_tasks t ON t.task_id IN (m.new_task, m.old_task));
    END IF;
    IF cardinality(keys) > 0 THEN PERFORM refresh_user_context_counters(keys); END IF;
    RETURN NULL;
END $$;

DROP TRIGGER IF EXISTS user_context_counters_task_insert ON agent_tasks;
CREATE TRIGGER user_context_counters_task_insert
    AFTER INSERT ON agent_tasks
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION user_context_counters_task_inserted();

DROP TRIGGER IF EXISTS user_context_counters_task_update ON agent_tasks;
CREATE TRIGGER user_context_counters_task_update
    AFTER UPDATE ON agent_tasks
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION user_context_counters_task_changed();

DROP TRIGGER IF EXISTS user_context_counters_task_delete ON agent_tasks;
CREATE TRIGGER user_context_counters_task_delete
    AFTER DELETE ON agent_tasks
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION user_context_counters_task_changed();

DROP TRIGGER IF EXISTS user_context_counters_message_insert ON task_messages;
CREATE TRIGGER user_context_counters_message_insert
    AFTER INSERT ON task_messages
    REFERENCING NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION user_context_counters_message_inserted();

DROP TRIGGER IF EXISTS user_context_counters_message_update ON task_messages;
CREATE TRIGGER user_context_counters_message_update
    AFTER UPDATE ON task_messages
    REFERENCING OLD TABLE AS old_rows NEW TABLE AS new_rows
    FOR EACH STATEMENT EXECUTE FUNCTION user_context_counters_message_changed();

DROP TRIGGER IF EXISTS user_context_counters_message_delete ON task_messages;
CREATE TRIGGER user_context_counters_message_delete
    AFTER DELETE ON task_messages
    REFERENCING OLD TABLE AS old_rows
    FOR EACH STATEMENT EXECUTE FUNCTION user_context_counters_message_changed();
