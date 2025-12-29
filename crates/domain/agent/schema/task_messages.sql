CREATE TABLE IF NOT EXISTS task_messages (
    id SERIAL PRIMARY KEY,

    task_id TEXT NOT NULL,

    message_id TEXT NOT NULL,
    client_message_id TEXT,
    role TEXT NOT NULL CHECK (role IN ('user', 'agent')),

    context_id TEXT,

    user_id TEXT,
    session_id TEXT,
    trace_id TEXT,

    sequence_number INTEGER NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    metadata JSONB DEFAULT '{}',

    reference_task_ids TEXT[],

    FOREIGN KEY (task_id) REFERENCES agent_tasks(task_id) ON DELETE CASCADE,
    UNIQUE(task_id, message_id),
    UNIQUE(message_id, task_id),
    UNIQUE(task_id, sequence_number)
);

CREATE INDEX IF NOT EXISTS idx_task_messages_task_id ON task_messages(task_id);
CREATE INDEX IF NOT EXISTS idx_task_messages_message_id ON task_messages(message_id);
CREATE INDEX IF NOT EXISTS idx_task_messages_sequence ON task_messages(task_id, sequence_number);
CREATE INDEX IF NOT EXISTS idx_task_messages_client_id ON task_messages(client_message_id) WHERE client_message_id IS NOT NULL;

CREATE INDEX IF NOT EXISTS idx_task_messages_user_id ON task_messages(user_id);
CREATE INDEX IF NOT EXISTS idx_task_messages_session_id ON task_messages(session_id);
CREATE INDEX IF NOT EXISTS idx_task_messages_trace_id ON task_messages(trace_id);
