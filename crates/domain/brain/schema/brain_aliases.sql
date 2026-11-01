CREATE EXTENSION IF NOT EXISTS pg_trgm;

CREATE TABLE IF NOT EXISTS brain_aliases (
    id TEXT PRIMARY KEY,
    subject_kind TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    alias TEXT NOT NULL,
    alias_norm TEXT NOT NULL CHECK (length(alias_norm) BETWEEN 1 AND 255),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE (subject_kind, alias_norm),
    FOREIGN KEY (subject_kind, subject_id)
        REFERENCES brain_subjects (subject_kind, subject_id) ON DELETE CASCADE
);

CREATE INDEX IF NOT EXISTS idx_brain_aliases_subject
    ON brain_aliases (subject_kind, subject_id);
CREATE INDEX IF NOT EXISTS idx_brain_aliases_trgm
    ON brain_aliases USING GIN (alias_norm gin_trgm_ops);
