-- Search term statistics (recall redesign): the document frequency of
-- every lexeme in search_weighted, refreshed by the consumer sweep, so the
-- full-text tier requires the informative words of a message and only
-- ranks by the common ones.

CREATE TABLE IF NOT EXISTS brain_search_terms (
    word TEXT PRIMARY KEY,
    ndoc BIGINT NOT NULL CHECK (ndoc >= 0),
    refreshed_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
