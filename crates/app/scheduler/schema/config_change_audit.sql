-- Every database change a config plane apply made, one row per entity.
--
-- `before_value` / `after_value` are the canonical values read back from the
-- database around the apply (NULL = absent), so a row records what actually
-- changed rather than what was asked for. Written by the reconciler, the
-- boot seed and `admin config planes apply`.
CREATE TABLE IF NOT EXISTS config_change_audit (
    id BIGSERIAL PRIMARY KEY,
    plane TEXT NOT NULL CHECK (length(plane) BETWEEN 1 AND 64),
    entity TEXT NOT NULL,
    change_kind TEXT NOT NULL CHECK (change_kind IN ('missing', 'changed', 'extra')),
    before_value TEXT,
    after_value TEXT,
    mode TEXT NOT NULL CHECK (mode IN ('insert_only', 'overwrite')),
    actor TEXT NOT NULL,
    reason TEXT NOT NULL,
    declared_hash TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);
CREATE INDEX IF NOT EXISTS idx_config_change_audit_plane_created
    ON config_change_audit(plane, created_at DESC);
