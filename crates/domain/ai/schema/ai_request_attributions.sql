-- The value one gateway request is attributed to in each tenant-registered
-- subject dimension (`project`, `cost_centre`, ...). Core owns no
-- organisation model: `dimension` is a SubjectAttributeProvider slug and
-- `value` is that provider's vocabulary. `source` records which signal
-- decided the value: the x-systemprompt-scope-<dimension> header, the API
-- key's bound value, or the provider's first (default) value.
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
