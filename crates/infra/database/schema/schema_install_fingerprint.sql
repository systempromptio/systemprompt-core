-- One row: the fingerprint of the declarative schema, migrations and
-- retirements that the last complete install applied. An install under the
-- bootstrap lock that finds the same fingerprint, a current ledger and every
-- owned table present applies no DDL.
CREATE TABLE IF NOT EXISTS schema_install_fingerprint (
    singleton BOOLEAN PRIMARY KEY DEFAULT true CHECK (singleton),
    fingerprint TEXT NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL DEFAULT CURRENT_TIMESTAMP
);
