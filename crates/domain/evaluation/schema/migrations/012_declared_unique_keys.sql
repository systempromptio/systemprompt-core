-- Two UNIQUE keys the declarative schema declares but established databases
-- can lack: eval_budget_accounts(owner_id,operation_key) — a 0.51.0 fresh
-- install stamped migration 004 without running it, and 004 is where the
-- index came from — and eval_experiments(owner_id,id), which no migration
-- ever created. Each is created only when no unique index already covers
-- exactly those columns, whatever it was named (004 called its index
-- eval_budget_accounts_owner_operation), so no database ends up with two.
CREATE OR REPLACE FUNCTION eval_ensure_unique_key(tbl regclass, cols text[], idx name)
RETURNS void LANGUAGE plpgsql AS $$
BEGIN
    IF NOT EXISTS (
        SELECT 1 FROM pg_index i
        WHERE i.indrelid = tbl AND i.indisunique
          AND (SELECT array_agg(a.attname::text ORDER BY k.ord)
                 FROM unnest(i.indkey) WITH ORDINALITY AS k(attnum, ord)
                 JOIN pg_attribute a ON a.attrelid = i.indrelid AND a.attnum = k.attnum) = cols
    ) THEN
        EXECUTE format('CREATE UNIQUE INDEX %I ON %s (%s)', idx, tbl,
                       (SELECT string_agg(quote_ident(c), ', ') FROM unnest(cols) AS c));
    END IF;
END $$;
SELECT eval_ensure_unique_key('eval_budget_accounts', ARRAY['owner_id','operation_key'], 'eval_budget_accounts_owner_id_operation_key_key');
SELECT eval_ensure_unique_key('eval_experiments', ARRAY['owner_id','id'], 'eval_experiments_owner_id_id_key');
DROP FUNCTION eval_ensure_unique_key(regclass, text[], name);
