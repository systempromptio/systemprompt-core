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
