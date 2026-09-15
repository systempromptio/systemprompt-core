-- Fresh databases receive the current managed-workspace authority directly.
-- Migration 005 performs the verified conversion only for databases that
-- started with the retired eval_frozen_workspaces table.
CREATE EXTENSION IF NOT EXISTS pgcrypto;
CREATE TABLE IF NOT EXISTS eval_managed_workspace_projections (
    owner_id TEXT NOT NULL,
    digest TEXT NOT NULL CHECK(digest ~ '^[0-9a-f]{64}$'),
    managed_revision_id TEXT NOT NULL,
    publication_generation BIGINT,
    manifest JSONB NOT NULL,
    verified_file_count INTEGER NOT NULL CHECK(verified_file_count BETWEEN 0 AND 256),
    verified_byte_count BIGINT NOT NULL CHECK(verified_byte_count BETWEEN 0 AND 8388608),
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    PRIMARY KEY(owner_id,digest)
);

CREATE OR REPLACE FUNCTION reject_eval_managed_workspace_change() RETURNS trigger AS $$
BEGIN RAISE EXCEPTION 'managed evaluator workspace projections are immutable' USING ERRCODE='23514'; END $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS eval_managed_workspace_projection_immutable ON eval_managed_workspace_projections;
CREATE TRIGGER eval_managed_workspace_projection_immutable BEFORE UPDATE ON eval_managed_workspace_projections FOR EACH ROW EXECUTE FUNCTION reject_eval_managed_workspace_change();
