CREATE TABLE IF NOT EXISTS analytics_fact_changes (
    owner_id TEXT NOT NULL REFERENCES users(id),
    change_id TEXT NOT NULL,
    fact_kind TEXT NOT NULL CHECK (fact_kind IN ('invocation','request','assessment','resource_association')),
    source TEXT NOT NULL CHECK (length(source) BETWEEN 1 AND 128),
    fact_id TEXT NOT NULL,
    revision BIGINT NOT NULL CHECK (revision > 0),
    occurred_at TIMESTAMPTZ NOT NULL,
    recorded_at TIMESTAMPTZ NOT NULL,
    payload JSONB,
    payload_digest TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending' CHECK (state IN ('pending','leased','applied','superseded')),
    lease_worker TEXT,
    lease_epoch BIGINT NOT NULL DEFAULT 0,
    lease_until TIMESTAMPTZ,
    attempts INTEGER NOT NULL DEFAULT 0,
    next_attempt_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    last_error TEXT,
    applied_at TIMESTAMPTZ,
    PRIMARY KEY(owner_id,change_id),
    UNIQUE(owner_id,fact_kind,source,fact_id,revision)
);
CREATE INDEX IF NOT EXISTS analytics_fact_pending ON analytics_fact_changes(owner_id,next_attempt_at,recorded_at,change_id) WHERE state IN ('pending','leased');
