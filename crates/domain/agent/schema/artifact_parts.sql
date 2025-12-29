CREATE TABLE IF NOT EXISTS artifact_parts (
    id SERIAL PRIMARY KEY,

    artifact_id TEXT NOT NULL,
    context_id TEXT NOT NULL,

    part_kind TEXT NOT NULL CHECK (part_kind IN ('text', 'file', 'data')),

    sequence_number INTEGER NOT NULL,

    text_content TEXT,

    file_name TEXT,
    file_mime_type TEXT,
    file_uri TEXT,
    file_bytes TEXT,

    data_content JSONB,

    metadata JSONB DEFAULT '{}',

    FOREIGN KEY (context_id, artifact_id) REFERENCES task_artifacts(context_id, artifact_id) ON DELETE CASCADE,
    UNIQUE(artifact_id, sequence_number),

    CONSTRAINT check_text_part
        CHECK (part_kind != 'text' OR text_content IS NOT NULL),
    CONSTRAINT check_file_part
        CHECK (part_kind != 'file' OR (file_uri IS NOT NULL OR file_bytes IS NOT NULL)),
    CONSTRAINT check_data_part
        CHECK (part_kind != 'data' OR data_content IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_artifact_parts_artifact_id ON artifact_parts(artifact_id);
CREATE INDEX IF NOT EXISTS idx_artifact_parts_context_id ON artifact_parts(context_id);
CREATE INDEX IF NOT EXISTS idx_artifact_parts_kind ON artifact_parts(part_kind);
CREATE INDEX IF NOT EXISTS idx_artifact_parts_sequence ON artifact_parts(artifact_id, sequence_number);
