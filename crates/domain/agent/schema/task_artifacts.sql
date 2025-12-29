CREATE TABLE IF NOT EXISTS task_artifacts (
    id SERIAL PRIMARY KEY,

    task_id TEXT NOT NULL,

    context_id TEXT,

    artifact_id TEXT NOT NULL,

    name TEXT,
    description TEXT,

    artifact_type TEXT NOT NULL,
    source TEXT,
    tool_name TEXT,
    mcp_execution_id TEXT,
    fingerprint TEXT,

    skill_id TEXT,
    skill_name TEXT,

    metadata JSONB DEFAULT '{}',

    created_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,

    FOREIGN KEY (task_id) REFERENCES agent_tasks(task_id) ON DELETE CASCADE,
    FOREIGN KEY (mcp_execution_id) REFERENCES mcp_tool_executions(mcp_execution_id) ON DELETE SET NULL,
    UNIQUE(task_id, artifact_id),
    UNIQUE(context_id, artifact_id)
);

CREATE INDEX IF NOT EXISTS idx_task_artifacts_task_id ON task_artifacts(task_id);
CREATE INDEX IF NOT EXISTS idx_task_artifacts_artifact_id ON task_artifacts(artifact_id);
CREATE INDEX IF NOT EXISTS idx_task_artifacts_artifact_type ON task_artifacts(artifact_type);
CREATE INDEX IF NOT EXISTS idx_task_artifacts_tool_name ON task_artifacts(tool_name);
CREATE INDEX IF NOT EXISTS idx_task_artifacts_mcp_execution_id ON task_artifacts(mcp_execution_id);
CREATE INDEX IF NOT EXISTS idx_task_artifacts_fingerprint ON task_artifacts(fingerprint);
CREATE INDEX IF NOT EXISTS idx_task_artifacts_context_id ON task_artifacts(context_id);
CREATE INDEX IF NOT EXISTS idx_task_artifacts_context_type ON task_artifacts(context_id, artifact_type);
CREATE INDEX IF NOT EXISTS idx_task_artifacts_skill_id ON task_artifacts(skill_id);
CREATE INDEX IF NOT EXISTS idx_task_artifacts_skill_name ON task_artifacts(skill_name);
