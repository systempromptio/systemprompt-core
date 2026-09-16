CREATE TABLE IF NOT EXISTS eval_resource_revisions (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    resource_kind TEXT NOT NULL CHECK(resource_kind IN ('case','dataset','rubric','policy')),
    resource_key TEXT NOT NULL,
    digest TEXT NOT NULL,
    content JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id,resource_kind,resource_key,digest),
    UNIQUE(owner_id,id)
);
