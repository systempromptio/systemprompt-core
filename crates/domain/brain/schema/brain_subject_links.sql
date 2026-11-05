CREATE TABLE IF NOT EXISTS brain_subject_links (
    subject_kind TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    other_kind TEXT NOT NULL,
    other_id TEXT NOT NULL,
    via_kind TEXT NOT NULL,
    fact_id TEXT NOT NULL REFERENCES brain_facts(id) ON DELETE CASCADE,
    PRIMARY KEY (subject_kind, subject_id, other_kind, other_id, fact_id)
);

CREATE INDEX IF NOT EXISTS idx_brain_subject_links_fact
    ON brain_subject_links (fact_id);
