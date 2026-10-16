-- Last reconcile outcome per declared configuration plane (`config_reconcile`).
--
-- One row per registered plane, rewritten on every pass: the authority the
-- plane ran under, the hash of its declaration, how many rows still differ
-- from it and what the pass did. `/readyz` and `admin config planes list`
-- read it, so every replica reports the same state whichever one ran.
CREATE TABLE IF NOT EXISTS config_plane_state (
    plane TEXT PRIMARY KEY CHECK (length(plane) BETWEEN 1 AND 64),
    authority TEXT NOT NULL CHECK (authority IN ('file', 'db', 'undeclared')),
    declared_hash TEXT,
    drift_rows INTEGER NOT NULL DEFAULT 0 CHECK (drift_rows >= 0),
    outcome TEXT NOT NULL
        CHECK (outcome IN ('in_sync', 'corrected', 'seeded', 'drift_reported', 'undeclared', 'failed')),
    last_error TEXT,
    reconciled_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    corrected_at TIMESTAMPTZ
);
