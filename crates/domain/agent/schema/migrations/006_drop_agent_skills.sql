-- Phase 2b: skills migrate from Postgres to disk.
-- task_execution_steps stores skill_id as opaque text inside step_content JSONB
-- with no foreign key to agent_skills, so dropping the table is safe.
DROP TABLE IF EXISTS agent_skills CASCADE;
