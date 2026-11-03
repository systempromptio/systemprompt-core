-- Recall redesign, write side: the working set and fact links read by
-- recall, a weighted search vector beside the plain one, the observed_at
-- column for as-of reads, and the synonyms config document.
--
-- The structure only. Every stored fact is queued for the brain_fact_index
-- projection, which the consumer sweep drains: it floors observed_at to
-- created_at, and writes the fact's working-set rows, links, aliases and
-- weighted search vector through the same code the write path uses, so an
-- upgraded database and a fresh install reach one state by one path.

ALTER TABLE brain_config_docs DROP CONSTRAINT IF EXISTS brain_config_docs_id_check;
ALTER TABLE brain_config_docs
    ADD CONSTRAINT brain_config_docs_id_check
    CHECK (id IN ('filters', 'importance', 'retrieval', 'synonyms')) NOT VALID;
ALTER TABLE brain_config_docs VALIDATE CONSTRAINT brain_config_docs_id_check;

ALTER TABLE brain_facts
    ADD COLUMN IF NOT EXISTS observed_at TIMESTAMPTZ NOT NULL DEFAULT now();

ALTER TABLE brain_facts
    ADD COLUMN IF NOT EXISTS search_weighted TSVECTOR NOT NULL DEFAULT ''::tsvector;
CREATE INDEX IF NOT EXISTS idx_brain_facts_search_weighted
    ON brain_facts USING GIN (search_weighted);

CREATE TABLE IF NOT EXISTS brain_working_set (
    scope_key TEXT NOT NULL,
    fact_id TEXT NOT NULL REFERENCES brain_facts(id) ON DELETE CASCADE,
    importance_base FLOAT8 NOT NULL,
    PRIMARY KEY (scope_key, fact_id)
);

CREATE INDEX IF NOT EXISTS brain_working_set_scope_idx
    ON brain_working_set (scope_key, importance_base DESC, fact_id);

CREATE TABLE IF NOT EXISTS brain_fact_links (
    fact_id TEXT NOT NULL REFERENCES brain_facts(id) ON DELETE CASCADE,
    subject_kind TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    via_field TEXT NOT NULL,
    PRIMARY KEY (fact_id, subject_kind, subject_id, via_field)
);

CREATE INDEX IF NOT EXISTS idx_brain_fact_links_subject
    ON brain_fact_links (subject_kind, subject_id);

INSERT INTO projection_backlog (projection, key)
SELECT 'brain_fact_index', id FROM brain_facts
ON CONFLICT DO NOTHING;
