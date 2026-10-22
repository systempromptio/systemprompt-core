-- The three-way-merge base per plane entity: the declared hash and origin the
-- last apply wrote. An apply compares the file's hash and the database's
-- current hash against this one to tell a deploy (file moved), drift (database
-- moved) and a conflict (both moved).
CREATE TABLE IF NOT EXISTS config_plane_base (
    plane TEXT NOT NULL CHECK (length(plane) BETWEEN 1 AND 64),
    entity TEXT NOT NULL CHECK (length(entity) > 0),
    declared_hash TEXT NOT NULL,
    source TEXT NOT NULL CHECK (source ~ '^(code:.+|bundle:.+|console:.+|migration)$'),
    applied_by TEXT NOT NULL,
    applied_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY (plane, entity)
);
