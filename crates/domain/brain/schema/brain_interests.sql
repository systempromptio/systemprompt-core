CREATE TABLE IF NOT EXISTS brain_interests (
    id TEXT PRIMARY KEY CHECK (id ~ '^[a-z0-9_.-]{1,64}$'),
    owner_user_id TEXT REFERENCES users(id) ON DELETE CASCADE,
    body JSONB NOT NULL,
    expires_at TIMESTAMPTZ,
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_brain_interests_owner
    ON brain_interests (owner_user_id) WHERE owner_user_id IS NOT NULL;
