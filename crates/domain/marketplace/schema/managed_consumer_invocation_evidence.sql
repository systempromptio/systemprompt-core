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
