CREATE TABLE IF NOT EXISTS eval_managed_workspace_assets (
    owner_id TEXT NOT NULL,
    workspace_digest TEXT NOT NULL CHECK(workspace_digest ~ '^[0-9a-f]{64}$'),
    path TEXT NOT NULL,
    asset_digest TEXT NOT NULL CHECK(asset_digest ~ '^[0-9a-f]{64}$'),
    content BYTEA NOT NULL,
    executable BOOLEAN NOT NULL DEFAULT FALSE,
    PRIMARY KEY(owner_id,workspace_digest,path),
    FOREIGN KEY(owner_id,workspace_digest) REFERENCES eval_managed_workspace_projections(owner_id,digest)
);
DROP TRIGGER IF EXISTS eval_managed_workspace_assets_immutable ON eval_managed_workspace_assets;
CREATE TRIGGER eval_managed_workspace_assets_immutable BEFORE UPDATE ON eval_managed_workspace_assets FOR EACH ROW EXECUTE FUNCTION reject_eval_managed_workspace_change();
