CREATE TABLE IF NOT EXISTS brain_fact_links (
    fact_id TEXT NOT NULL REFERENCES brain_facts(id) ON DELETE CASCADE,
    subject_kind TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    via_field TEXT NOT NULL,
    PRIMARY KEY (fact_id, subject_kind, subject_id, via_field)
);

CREATE INDEX IF NOT EXISTS idx_brain_fact_links_subject
    ON brain_fact_links (subject_kind, subject_id);
