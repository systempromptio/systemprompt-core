-- One bound value per tenant scope dimension for an API key: it attributes
-- the key's gateway requests when they carry no
-- x-systemprompt-scope-<dimension> header.
CREATE TABLE IF NOT EXISTS user_api_key_scopes (
    key_id TEXT NOT NULL REFERENCES user_api_keys(id) ON DELETE CASCADE,
    dimension TEXT NOT NULL CHECK (length(dimension) BETWEEN 1 AND 64),
    value TEXT NOT NULL CHECK (length(value) > 0),
    PRIMARY KEY (key_id, dimension)
);
