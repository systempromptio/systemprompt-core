CREATE TABLE IF NOT EXISTS managed_invocation_attributions (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL,
    invocation_id TEXT NOT NULL,
    installation_id TEXT,
    resource_id TEXT,
    revision_id TEXT,
    publication_generation BIGINT,
    traffic_class TEXT NOT NULL CHECK (traffic_class IN ('production','fixture')),
    status TEXT NOT NULL CHECK (status IN ('verified','revision_unknown','unsupported','historical')),
    receipt_id TEXT,
    authenticated_evidence JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    FOREIGN KEY(owner_id, receipt_id) REFERENCES managed_installation_receipts(owner_id, id),
    UNIQUE(owner_id, invocation_id)
);
DROP TRIGGER IF EXISTS managed_invocation_attributions_immutable ON managed_invocation_attributions;
CREATE TRIGGER managed_invocation_attributions_immutable BEFORE UPDATE ON managed_invocation_attributions FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
