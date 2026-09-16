CREATE TABLE IF NOT EXISTS eval_fixture_payloads (
    owner_id TEXT NOT NULL REFERENCES users(id),
    fixture_key TEXT NOT NULL,
    digest TEXT NOT NULL,
    payload JSONB NOT NULL,
    evidence_label TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(owner_id,fixture_key,digest)
);
