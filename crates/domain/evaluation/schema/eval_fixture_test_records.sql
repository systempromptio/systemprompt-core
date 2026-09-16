CREATE TABLE IF NOT EXISTS eval_fixture_test_records (
    owner_id TEXT NOT NULL REFERENCES users(id),
    record_key TEXT NOT NULL,
    value JSONB NOT NULL,
    original_value JSONB NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(owner_id,record_key)
);
