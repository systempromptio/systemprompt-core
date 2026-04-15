ALTER TABLE ai_requests ALTER COLUMN task_id TYPE TEXT;

UPDATE ai_requests
SET task_id = NULL
WHERE task_id IS NOT NULL
  AND task_id NOT IN (SELECT task_id FROM agent_tasks);

ALTER TABLE ai_requests
    ADD CONSTRAINT fk_ai_requests_task_id
    FOREIGN KEY (task_id) REFERENCES agent_tasks(task_id)
    ON DELETE SET NULL;
