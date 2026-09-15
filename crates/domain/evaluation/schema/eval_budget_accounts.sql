CREATE TABLE IF NOT EXISTS eval_budget_accounts (
    id TEXT PRIMARY KEY,
    owner_id TEXT NOT NULL REFERENCES users(id),
    cap BIGINT NOT NULL CHECK(cap>0),
    reserved BIGINT NOT NULL DEFAULT 0 CHECK(reserved>=0),
    settled BIGINT NOT NULL DEFAULT 0 CHECK(settled>=0),
    frozen BOOLEAN NOT NULL DEFAULT FALSE,
    operation_key TEXT NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    UNIQUE(owner_id,operation_key),
    UNIQUE(owner_id,id)
);
