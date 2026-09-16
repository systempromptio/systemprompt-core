CREATE TABLE IF NOT EXISTS managed_api_operations (
    owner_id TEXT NOT NULL REFERENCES users(id),
    id TEXT NOT NULL,
    kind TEXT NOT NULL,
    request_digest TEXT NOT NULL,
    state TEXT NOT NULL DEFAULT 'pending' CHECK(state IN ('pending','completed','failed')),
    fence BIGINT NOT NULL DEFAULT 1,
    lease_until TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp()+interval '180 seconds',
    input_checkpoint JSONB,
    result JSONB,
    problem TEXT,
    created_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT clock_timestamp(),
    PRIMARY KEY(owner_id,id)
);
ALTER TABLE managed_consumer_credentials ADD COLUMN IF NOT EXISTS issuance_operation TEXT;
