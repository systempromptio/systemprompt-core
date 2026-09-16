CREATE TABLE IF NOT EXISTS managed_consumer_grants (
    owner_id TEXT NOT NULL,
    resource_id TEXT NOT NULL,
    consumer_id TEXT NOT NULL REFERENCES users(id) ON DELETE CASCADE,
    granted_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    revoked_at TIMESTAMPTZ,
    PRIMARY KEY(owner_id, resource_id, consumer_id),
    FOREIGN KEY(owner_id, resource_id) REFERENCES managed_resources(owner_id, id)
);
CREATE UNIQUE INDEX IF NOT EXISTS managed_consumer_receipt_identity ON managed_installation_receipts(consumer_id, device_id, host, installation_id, publication_id) WHERE consumer_id IS NOT NULL;
