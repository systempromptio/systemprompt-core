-- Document frequency of every lexeme in brain_facts.search_weighted,
-- refreshed by the consumer sweep from ts_stat. The full-text tier plans
-- its query from it: a word that occurs in more than retrieval.yaml's
-- search_max_df share of the facts cannot narrow the candidate set and is
-- used only to rank; a word no fact carries is dropped rather than
-- required.
CREATE TABLE IF NOT EXISTS brain_search_terms (
    word TEXT PRIMARY KEY,
    ndoc BIGINT NOT NULL CHECK (ndoc >= 0),
    refreshed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
