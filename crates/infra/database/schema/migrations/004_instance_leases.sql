-- Instance leases (schema/instance_leases.sql): expiring single-owner claims on
-- a named resource, taken through the LeaseRepository seam. The table is the
-- base schema's text.
CREATE TABLE IF NOT EXISTS instance_leases (
    lease_key TEXT PRIMARY KEY,
    owner_instance_id TEXT NOT NULL,
    acquired_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP,
    expires_at TIMESTAMPTZ NOT NULL
);

CREATE INDEX IF NOT EXISTS idx_instance_leases_owner ON instance_leases(owner_instance_id);
