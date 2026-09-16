CREATE TABLE IF NOT EXISTS managed_consumer_credentials (
    device_id TEXT PRIMARY KEY REFERENCES user_device_certs(id) ON DELETE CASCADE,
    credential_digest TEXT NOT NULL UNIQUE CHECK (credential_digest ~ '^[0-9a-f]{64}$'),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ
);
CREATE TABLE IF NOT EXISTS managed_consumer_grants (
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    consumer_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ,
    PRIMARY KEY(owner_id, resource_id, consumer_id),
    FOREIGN KEY(owner_id, resource_id) REFERENCES managed_resources(owner_id, id)
);
ALTER TABLE managed_installation_receipts ADD COLUMN IF NOT EXISTS consumer_id TEXT REFERENCES users(id) ON DELETE CASCADE;
ALTER TABLE managed_installation_receipts ADD COLUMN IF NOT EXISTS device_id TEXT REFERENCES user_device_certs(id) ON DELETE CASCADE;
ALTER TABLE managed_installation_receipts ADD COLUMN IF NOT EXISTS host TEXT;
ALTER TABLE managed_installation_receipts ADD COLUMN IF NOT EXISTS consumer_evidence JSONB;
ALTER TABLE managed_installation_receipts ADD COLUMN IF NOT EXISTS fully_verified BOOLEAN;
CREATE UNIQUE INDEX IF NOT EXISTS managed_consumer_receipt_identity ON managed_installation_receipts(consumer_id, device_id, host, installation_id, publication_id) WHERE consumer_id IS NOT NULL;
CREATE TABLE IF NOT EXISTS managed_consumer_session_bindings (
    id TEXT PRIMARY KEY,
    receipt_id TEXT NOT NULL REFERENCES managed_installation_receipts(id) ON DELETE CASCADE,
    consumer_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL REFERENCES user_device_certs(id) ON DELETE CASCADE,
    host TEXT NOT NULL,
    native_session_id TEXT NOT NULL CHECK (length(native_session_id) BETWEEN 1 AND 512),
    bound_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(receipt_id, consumer_id, device_id, host, native_session_id)
);
CREATE TABLE IF NOT EXISTS managed_consumer_invocation_evidence (
    id TEXT PRIMARY KEY,
    consumer_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    device_id TEXT NOT NULL REFERENCES user_device_certs(id) ON DELETE CASCADE,
    host TEXT NOT NULL,
    native_session_id TEXT NOT NULL CHECK (length(native_session_id) BETWEEN 1 AND 512),
    invocation_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    installation_id TEXT,
    revision_id TEXT,
    generation BIGINT,
    evidence JSONB NOT NULL,
    occurred_at TIMESTAMPTZ NOT NULL,
    received_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    UNIQUE(consumer_id, device_id, host, invocation_id)
);
CREATE TABLE IF NOT EXISTS managed_consumer_attribution_projection (
    evidence_id TEXT PRIMARY KEY REFERENCES managed_consumer_invocation_evidence(id) ON DELETE CASCADE,
    receipt_id TEXT REFERENCES managed_installation_receipts(id) ON DELETE SET NULL,
    version BIGINT NOT NULL DEFAULT 1,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
CREATE TABLE IF NOT EXISTS managed_consumer_attribution_history (
    evidence_id TEXT NOT NULL REFERENCES managed_consumer_invocation_evidence(id) ON DELETE CASCADE,
    version BIGINT NOT NULL,
    receipt_id TEXT REFERENCES managed_installation_receipts(id) ON DELETE SET NULL,
    reason TEXT NOT NULL,
    corrected_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY(evidence_id, version)
);
CREATE INDEX IF NOT EXISTS managed_consumer_evidence_session ON managed_consumer_invocation_evidence(consumer_id, device_id, host, native_session_id);
DROP TRIGGER IF EXISTS managed_consumer_bindings_immutable ON managed_consumer_session_bindings;
CREATE TRIGGER managed_consumer_bindings_immutable BEFORE UPDATE ON managed_consumer_session_bindings FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
DROP TRIGGER IF EXISTS managed_consumer_evidence_immutable ON managed_consumer_invocation_evidence;
CREATE TRIGGER managed_consumer_evidence_immutable BEFORE UPDATE ON managed_consumer_invocation_evidence FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
