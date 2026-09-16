CREATE TABLE IF NOT EXISTS eval_budget_reservations (
    id TEXT PRIMARY KEY,
    account_id TEXT NOT NULL REFERENCES eval_budget_accounts(id),
    operation_key TEXT NOT NULL,
    reserved BIGINT NOT NULL CHECK(reserved>0),
    actual BIGINT CHECK(actual>=0),
    request_id TEXT UNIQUE,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW(),
    settled_at TIMESTAMPTZ,
    UNIQUE(account_id,operation_key)
);
