-- Recall redesign, write side: the working set and fact links read by
-- recall, a weighted search vector beside the plain one, the observed_at
-- column for as-of reads, and the synonyms config document.

ALTER TABLE brain_config_docs DROP CONSTRAINT IF EXISTS brain_config_docs_id_check;
ALTER TABLE brain_config_docs
    ADD CONSTRAINT brain_config_docs_id_check
    CHECK (id IN ('filters', 'importance', 'retrieval', 'synonyms'));

ALTER TABLE brain_facts
    ADD COLUMN IF NOT EXISTS observed_at TIMESTAMPTZ NOT NULL DEFAULT now();
-- Why: Postgres fills a new DEFAULT now() column with the migration's time;
-- a fact observed before it was recorded is impossible, so created_at is the
-- closest truth for rows that predate the column.
UPDATE brain_facts SET observed_at = created_at WHERE observed_at > created_at;

ALTER TABLE brain_facts
    ADD COLUMN IF NOT EXISTS search_weighted TSVECTOR NOT NULL DEFAULT ''::tsvector;
CREATE INDEX IF NOT EXISTS idx_brain_facts_search_weighted
    ON brain_facts USING GIN (search_weighted);
UPDATE brain_facts SET search_weighted = setweight(to_tsvector('simple', statement), 'C')
    WHERE search_weighted = ''::tsvector;

CREATE TABLE IF NOT EXISTS brain_working_set (
    scope_key TEXT NOT NULL,
    fact_id TEXT NOT NULL REFERENCES brain_facts(id) ON DELETE CASCADE,
    importance_base FLOAT8 NOT NULL,
    PRIMARY KEY (scope_key, fact_id)
);

CREATE INDEX IF NOT EXISTS brain_working_set_scope_idx
    ON brain_working_set (scope_key, importance_base DESC, fact_id);

INSERT INTO brain_working_set (scope_key, fact_id, importance_base)
SELECT DISTINCT s.key, f.id, f.importance_base
FROM brain_facts f
CROSS JOIN LATERAL (
    SELECT CASE WHEN f.scope_level = 'org' THEN 'org'
                ELSE f.scope_level || ':' || f.scope_key END AS key
    UNION
    SELECT CASE WHEN a = 'everyone' THEN 'org' ELSE a END
    FROM brain_topics t,
         jsonb_array_elements_text(COALESCE(t.body -> 'audience', '[]'::jsonb)) a
    WHERE t.id = ANY (f.topics)
) s
ON CONFLICT DO NOTHING;

CREATE TABLE IF NOT EXISTS brain_fact_links (
    fact_id TEXT NOT NULL REFERENCES brain_facts(id) ON DELETE CASCADE,
    subject_kind TEXT NOT NULL,
    subject_id TEXT NOT NULL,
    via_field TEXT NOT NULL,
    PRIMARY KEY (fact_id, subject_kind, subject_id, via_field)
);

CREATE INDEX IF NOT EXISTS idx_brain_fact_links_subject
    ON brain_fact_links (subject_kind, subject_id);

INSERT INTO brain_fact_links (fact_id, subject_kind, subject_id, via_field)
SELECT f.id, f.subject_kind, f.subject_id, 'subject' FROM brain_facts f
UNION
SELECT f.id,
       CASE WHEN strpos(x.v, ':') > 0 THEN split_part(x.v, ':', 1) ELSE d.decl ->> 'ref' END,
       CASE WHEN strpos(x.v, ':') > 0 THEN substr(x.v, strpos(x.v, ':') + 1) ELSE x.v END,
       d.key
FROM brain_facts f
JOIN brain_fact_kinds k ON k.id = f.kind
CROSS JOIN LATERAL jsonb_each(COALESCE(k.body -> 'fields', '{}'::jsonb)) d(key, decl)
CROSS JOIN LATERAL (SELECT f.fields ->> d.key AS v) x
WHERE d.decl ->> 'type' = 'entity'
  AND x.v IS NOT NULL AND x.v <> ''
  AND (strpos(x.v, ':') > 0 OR d.decl ->> 'ref' IS NOT NULL)
ON CONFLICT DO NOTHING;
