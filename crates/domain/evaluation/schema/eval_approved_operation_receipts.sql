CREATE TABLE IF NOT EXISTS eval_approved_operation_receipts (
    approval_id TEXT PRIMARY KEY REFERENCES eval_execution_approvals(id),
    execution_id TEXT NOT NULL REFERENCES eval_executions(id),
    operation_digest TEXT NOT NULL CHECK(operation_digest ~ '^[0-9a-f]{64}$'),
    output JSONB NOT NULL,
    created_at TIMESTAMPTZ NOT NULL DEFAULT NOW()
);

CREATE OR REPLACE FUNCTION reject_eval_operation_receipt_change() RETURNS trigger AS $$
BEGIN RAISE EXCEPTION 'approved operation receipts are immutable' USING ERRCODE='23514'; END $$ LANGUAGE plpgsql;
DROP TRIGGER IF EXISTS eval_operation_receipts_immutable ON eval_approved_operation_receipts;
CREATE TRIGGER eval_operation_receipts_immutable BEFORE UPDATE OR DELETE ON eval_approved_operation_receipts FOR EACH ROW EXECUTE FUNCTION reject_eval_operation_receipt_change();
