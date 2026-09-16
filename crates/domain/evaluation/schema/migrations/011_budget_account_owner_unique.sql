-- Established databases created eval_budget_accounts before the declarative
-- schema gained UNIQUE(owner_id,id), so the owner-paired foreign key on
-- eval_experiments(owner_id,budget_id) could never be created there: no
-- unique index matched the referenced columns. The key is applied by the
-- installer's deferred foreign-key phase once this index exists.
CREATE UNIQUE INDEX IF NOT EXISTS eval_budget_accounts_owner_id_id_key
    ON eval_budget_accounts(owner_id,id);
