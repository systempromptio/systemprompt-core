-- Git dependency verification, its source bindings, the baseline-capture
-- ledger, the API operation ledger and the consumer invocation evidence with
-- its attribution projection and history were written only through the
-- managed HTTP admin routes removed in this release; managed invocation
-- attributions were never written at all. Their declarative schema is gone,
-- so the tables are dropped here, referencing tables first.
DROP TABLE IF EXISTS managed_git_verifications;
DROP TABLE IF EXISTS managed_dependency_verifications;
DROP TABLE IF EXISTS managed_resource_git_bindings;
DROP TABLE IF EXISTS managed_inventory_captures;

-- A consumer view on an older database may still read one of these (an
-- extension's view of invocation attribution did until 0.53), and a
-- consumer's migrations run after core's. Such a table is left for the
-- consumer to drop once it has redefined the view; every other one goes now.
DO $$
DECLARE
    retired TEXT;
BEGIN
    FOREACH retired IN ARRAY ARRAY[
        'managed_invocation_attributions',
        'managed_consumer_attribution_history',
        'managed_consumer_attribution_projection',
        'managed_consumer_invocation_evidence',
        'managed_api_operations'
    ] LOOP
        CONTINUE WHEN to_regclass(retired) IS NULL;
        IF EXISTS (
            SELECT 1
            FROM pg_depend d
            JOIN pg_rewrite r ON d.classid = 'pg_rewrite'::regclass AND r.oid = d.objid
            WHERE d.refobjid = to_regclass(retired) AND r.ev_class <> to_regclass(retired)
        ) THEN
            RAISE NOTICE '% is still read by a view; its consumer drops it', retired;
            CONTINUE;
        END IF;
        EXECUTE format('DROP TABLE %I', retired);
    END LOOP;
END $$;
