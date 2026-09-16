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
