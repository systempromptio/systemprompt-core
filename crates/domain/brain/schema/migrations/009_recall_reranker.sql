-- Reranker seam: a recall receipt names the reranker that ordered its
-- visible facts and the rank it gave each injected one, as one JSON
-- document {reranker, ranks: [{id, rank}]} beside the injected refs. NULL
-- when no reranker ran. A nullable column with no default is a catalogue
-- change only; no row is rewritten.
-- @cost: rows=0 measured=0.0s triggers=live

ALTER TABLE brain_recalls ADD COLUMN IF NOT EXISTS reranked JSONB;
