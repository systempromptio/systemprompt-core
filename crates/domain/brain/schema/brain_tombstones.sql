CREATE TABLE IF NOT EXISTS brain_tombstones (
    kind TEXT NOT NULL,
    identity_hash TEXT NOT NULL,
    erased_by TEXT REFERENCES users(id) ON DELETE SET NULL,
    erased_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (kind, identity_hash)
);
