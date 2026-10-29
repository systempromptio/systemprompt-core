CREATE TABLE IF NOT EXISTS notification_streams (
    id TEXT PRIMARY KEY CHECK (id ~ '^[a-z0-9_-]{1,64}$'),
    name TEXT NOT NULL,
    description TEXT,
    audience TEXT[] NOT NULL,
    publishers TEXT[] NOT NULL,
    attention_default TEXT NOT NULL CHECK (attention_default IN ('silent', 'agent', 'human')),
    attention_floor TEXT CHECK (attention_floor IN ('silent', 'agent', 'human')),
    attention_ceiling TEXT CHECK (attention_ceiling IN ('silent', 'agent', 'human')),
    retention_days INT CHECK (retention_days > 0),
    coalesce_window_secs INT CHECK (coalesce_window_secs >= 0),
    source TEXT NOT NULL,
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now()
);
