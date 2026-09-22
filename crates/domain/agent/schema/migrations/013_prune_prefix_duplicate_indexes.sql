-- Drop every index on this crate's tables whose column list is a strict
-- prefix of another, non-partial index on the same table. A prefix index
-- answers no query its superset cannot; it only costs a write on every
-- insert. The A2A part tables carried three indexes on one key apiece.
--
-- Each one names a FULL (non-partial) index that covers it — a partial
-- superset would not be a safe replacement, so none is cited here. None of
-- the dropped indexes is UNIQUE and none backs a constraint.

-- covered by idx_agent_tasks_context_status
DROP INDEX IF EXISTS idx_agent_tasks_context_id;
-- covered by idx_agent_tasks_user_created
DROP INDEX IF EXISTS idx_agent_tasks_user_id;

-- covered by idx_artifact_parts_sequence, itself covered by
-- artifact_parts_artifact_id_sequence_number_key (same columns)
DROP INDEX IF EXISTS idx_artifact_parts_artifact_id;
DROP INDEX IF EXISTS idx_artifact_parts_sequence;

-- covered by context_agents_context_id_agent_name_key and idx_context_agents_active
DROP INDEX IF EXISTS idx_context_agents_context;

-- covered by idx_message_parts_sequence, itself covered by
-- message_parts_message_id_sequence_number_key (same columns)
DROP INDEX IF EXISTS idx_message_parts_message_id;
DROP INDEX IF EXISTS idx_message_parts_sequence;

-- covered by task_artifacts_context_id_artifact_id_key and idx_task_artifacts_context_type
DROP INDEX IF EXISTS idx_task_artifacts_context_id;
-- covered by task_artifacts_task_id_artifact_id_key
DROP INDEX IF EXISTS idx_task_artifacts_task_id;

-- covered by task_messages_message_id_task_id_key
DROP INDEX IF EXISTS idx_task_messages_message_id;
-- covered by task_messages_task_id_sequence_number_key (same columns)
DROP INDEX IF EXISTS idx_task_messages_sequence;
-- covered by task_messages_task_id_message_id_key and the two above it
DROP INDEX IF EXISTS idx_task_messages_task_id;

-- covered by idx_user_contexts_user_updated
DROP INDEX IF EXISTS idx_user_contexts_user;
