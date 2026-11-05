-- Recall depth (P4): a fact whose subject and one of whose entity fields
-- name two different subjects declares a bridge between them
-- (engagement_phase: client <-> project; deal_stage: client <-> deal), and
-- the linked tier at links_depth 2 reaches the facts about, and linked
-- to, a bridged subject. The bridge table is derived from brain_fact_links
-- both ways, kept by the set reindex and cascaded with the fact. Its
-- primary key leads with the subject the tier looks up; the second index
-- serves the delete by fact id. The structure only: every fact that
-- carries an entity link is queued for the brain_fact_index projection,
-- whose sweep writes its bridges through the same code the write path
-- uses, so an upgraded instance bridges once the sweep has drained.
-- @cost: rows=0 measured=0.4s triggers=live

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

INSERT INTO projection_backlog (projection, key)
SELECT DISTINCT 'brain_fact_index', l.fact_id
FROM brain_fact_links l
WHERE l.via_field <> 'subject'
ON CONFLICT DO NOTHING;
