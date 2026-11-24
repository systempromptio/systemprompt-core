CREATE SEQUENCE IF NOT EXISTS instance_lease_fencing_seq;

CREATE TABLE IF NOT EXISTS instance_leases (
    lease_key TEXT PRIMARY KEY,
    owner_instance_id TEXT NOT NULL,
    acquired_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL,
    fencing_token BIGINT NOT NULL DEFAULT 0
);

CREATE INDEX IF NOT EXISTS idx_instance_leases_owner ON instance_leases(owner_instance_id);
