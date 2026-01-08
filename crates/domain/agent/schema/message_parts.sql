CREATE TABLE IF NOT EXISTS message_parts (
    id SERIAL PRIMARY KEY,

    message_id TEXT NOT NULL,
    task_id TEXT NOT NULL,

    part_kind TEXT NOT NULL CHECK (part_kind IN ('text', 'file', 'data')),

    sequence_number INTEGER NOT NULL,

    text_content TEXT,

    file_name TEXT,
    file_mime_type TEXT,
    file_uri TEXT,
    file_bytes TEXT,
    file_id UUID,

    data_content JSONB,

    metadata JSONB DEFAULT '{}',

    FOREIGN KEY (message_id, task_id) REFERENCES task_messages(message_id, task_id) ON DELETE CASCADE,
    UNIQUE(message_id, sequence_number),

    CONSTRAINT check_text_part
        CHECK (part_kind != 'text' OR text_content IS NOT NULL),
    CONSTRAINT check_file_part
        CHECK (part_kind != 'file' OR (file_uri IS NOT NULL OR file_bytes IS NOT NULL)),
    CONSTRAINT check_data_part
        CHECK (part_kind != 'data' OR data_content IS NOT NULL)
);

CREATE INDEX IF NOT EXISTS idx_message_parts_message_id ON message_parts(message_id);
CREATE INDEX IF NOT EXISTS idx_message_parts_task_id ON message_parts(task_id);
CREATE INDEX IF NOT EXISTS idx_message_parts_kind ON message_parts(part_kind);
CREATE INDEX IF NOT EXISTS idx_message_parts_sequence ON message_parts(message_id, sequence_number);
CREATE INDEX IF NOT EXISTS idx_message_parts_file_id ON message_parts(file_id) WHERE file_id IS NOT NULL;
