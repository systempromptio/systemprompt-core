CREATE TABLE IF NOT EXISTS managed_installation_receipts (
    id TEXT PRIMARY KEY,
    consumer_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    device_id TEXT REFERENCES user_device_certs(id) ON DELETE CASCADE,
    host TEXT,
    consumer_evidence JSONB,
    fully_verified BOOLEAN,
    owner_id TEXT NOT NULL,
    installation_id TEXT NOT NULL,
    publication_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    generation BIGINT NOT NULL CHECK (generation > 0),
    bundle_digest TEXT NOT NULL CHECK (bundle_digest ~ '^[0-9a-f]{64}$'),
    installed_manifest JSONB NOT NULL,
    client_evidence JSONB NOT NULL,
    verified_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, resource_id, publication_id) REFERENCES managed_publications(owner_id, resource_id, id),
    UNIQUE(owner_id, installation_id, resource_id, generation),
    UNIQUE(owner_id, id)
);
DROP TRIGGER IF EXISTS managed_installation_receipts_immutable ON managed_installation_receipts;
CREATE TRIGGER managed_installation_receipts_immutable BEFORE UPDATE ON managed_installation_receipts FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
