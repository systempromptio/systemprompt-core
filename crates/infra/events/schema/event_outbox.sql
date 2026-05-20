CREATE TABLE IF NOT EXISTS event_outbox (
    id TEXT PRIMARY KEY,
    channel TEXT NOT NULL,
    user_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'job', 'mcp')),
    actor_id TEXT NOT NULL CHECK (length(actor_id) > 0),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_event_outbox_created_at ON event_outbox(created_at);
CREATE INDEX IF NOT EXISTS idx_event_outbox_actor ON event_outbox(actor_kind, actor_id);
