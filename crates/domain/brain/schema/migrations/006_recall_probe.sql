-- Recall latency (P3): every retrieval tier admits a fact through a probe
-- of the working set by fact id, so the projection gains the index that
-- probe and the per-fact refresh read; the generated `search` column and
-- its GIN index on brain_facts were read only by the retired
-- scope-predicate tiers and go with them. Keys only: the index build
-- reads brain_working_set once (3 rows per fact at 300k facts, ~1 s),
-- the column drop is a catalogue change.
-- @cost: rows=0 measured=1.2s triggers=live

CREATE INDEX IF NOT EXISTS brain_working_set_fact_idx
    ON brain_working_set (fact_id, scope_key);

DROP INDEX IF EXISTS idx_brain_facts_search;

ALTER TABLE brain_facts DROP COLUMN IF EXISTS search;
