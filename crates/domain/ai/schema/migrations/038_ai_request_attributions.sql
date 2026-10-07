-- @supersedes-checksum: 8046193ebe98e890
-- @cost: rows=0 measured=40ms triggers=live
-- Scope attribution: the API key that authenticated a gateway request, and
-- one row per tenant-registered subject dimension the request is charged to.
ALTER TABLE ai_requests
    ADD COLUMN IF NOT EXISTS api_key_id TEXT;
CREATE INDEX IF NOT EXISTS idx_ai_requests_api_key_created ON ai_requests(api_key_id, created_at);

CREATE TABLE IF NOT EXISTS ai_request_attributions (
    request_id VARCHAR(255) NOT NULL REFERENCES ai_requests(id) ON DELETE CASCADE,
    dimension TEXT NOT NULL CHECK (length(dimension) BETWEEN 1 AND 64),
    value TEXT NOT NULL CHECK (length(value) > 0),
    source TEXT NOT NULL
        CONSTRAINT ai_request_attributions_source_check CHECK (source IN ('header', 'api_key', 'default')),
    PRIMARY KEY (request_id, dimension)
);
CREATE INDEX IF NOT EXISTS idx_ai_request_attributions_dimension_value
    ON ai_request_attributions(dimension, value);
