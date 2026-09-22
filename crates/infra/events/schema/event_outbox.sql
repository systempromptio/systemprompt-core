CREATE TABLE IF NOT EXISTS event_outbox (
    id TEXT PRIMARY KEY,
    channel TEXT NOT NULL,
    user_id TEXT NOT NULL,
    payload JSONB NOT NULL,
    actor_kind TEXT NOT NULL CHECK (actor_kind IN ('user', 'job', 'mcp')),
    actor_id TEXT NOT NULL CONSTRAINT event_outbox_actor_id_nonempty CHECK (length(actor_id) > 0),
    origin_instance_id TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    consumer TEXT,
    fact JSONB,
    processed_at TIMESTAMPTZ,
    deliver_to_origin BOOLEAN NOT NULL DEFAULT FALSE,
    CONSTRAINT event_outbox_fact_pair CHECK (
        (consumer IS NULL AND fact IS NULL AND processed_at IS NULL)
        OR (consumer IS NOT NULL AND length(consumer) > 0 AND fact IS NOT NULL)
    )
);

CREATE INDEX IF NOT EXISTS idx_event_outbox_created_at ON event_outbox(created_at);
CREATE INDEX IF NOT EXISTS idx_event_outbox_actor ON event_outbox(actor_kind, actor_id);
CREATE INDEX IF NOT EXISTS idx_event_outbox_pending
    ON event_outbox(consumer, created_at, id)
    WHERE consumer IS NOT NULL AND processed_at IS NULL;
