-- An entity whose file and database both moved since the last apply. The
-- apply leaves it untouched and records both sides here; `/admin/sync` shows
-- the open rows and records who resolved each and how.
CREATE TABLE IF NOT EXISTS config_conflicts (
    id BIGSERIAL PRIMARY KEY,
    plane TEXT NOT NULL CHECK (length(plane) BETWEEN 1 AND 64),
    entity TEXT NOT NULL CHECK (length(entity) > 0),
    base_hash TEXT,
    file_hash TEXT NOT NULL,
    file_value TEXT,
    db_hash TEXT NOT NULL,
    db_value TEXT,
    detected_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    resolved_by TEXT,
    resolved_at TIMESTAMPTZ,
    resolution TEXT CHECK (resolution IN ('file', 'db', 'edited')),
    CHECK ((resolved_at IS NULL) = (resolved_by IS NULL)),
    CHECK ((resolved_at IS NULL) = (resolution IS NULL))
);
CREATE UNIQUE INDEX IF NOT EXISTS idx_config_conflicts_open
    ON config_conflicts(plane, entity) WHERE resolved_at IS NULL;
