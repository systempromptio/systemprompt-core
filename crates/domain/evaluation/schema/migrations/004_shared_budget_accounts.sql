ALTER TABLE eval_budget_accounts ADD COLUMN IF NOT EXISTS operation_key TEXT;
UPDATE eval_budget_accounts SET operation_key='legacy-' || id WHERE operation_key IS NULL;
ALTER TABLE eval_budget_accounts ALTER COLUMN operation_key SET NOT NULL;
CREATE UNIQUE INDEX IF NOT EXISTS eval_budget_accounts_owner_operation
    ON eval_budget_accounts(owner_id, operation_key);
