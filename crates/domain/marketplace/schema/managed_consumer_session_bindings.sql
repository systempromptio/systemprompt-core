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
DROP TRIGGER IF EXISTS managed_consumer_bindings_immutable ON managed_consumer_session_bindings;
CREATE TRIGGER managed_consumer_bindings_immutable BEFORE UPDATE ON managed_consumer_session_bindings FOR EACH ROW EXECUTE FUNCTION reject_managed_content_update();
