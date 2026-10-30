CREATE TABLE IF NOT EXISTS brain_subjects (
    subject_kind TEXT NOT NULL CHECK (length(subject_kind) BETWEEN 1 AND 64),
    subject_id TEXT NOT NULL CHECK (length(subject_id) BETWEEN 1 AND 255),
    display_name TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (subject_kind, subject_id)
);
