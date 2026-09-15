CREATE TABLE IF NOT EXISTS managed_assets (
    owner_id TEXT NOT NULL REFERENCES users(id),
    digest TEXT NOT NULL CHECK (digest ~ '^[0-9a-f]{64}$'),
    content BYTEA NOT NULL CHECK (octet_length(content) <= 16777216),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(owner_id, digest)
);
DROP TRIGGER IF EXISTS managed_assets_immutable ON managed_assets;
CREATE TRIGGER managed_assets_immutable BEFORE UPDATE ON managed_assets FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
