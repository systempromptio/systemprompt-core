-- A local-tree source captured while bundle sources were composed recorded the
-- resolved composed tree, a directory keyed by its hash that rotates on every
-- import and is pruned soon after. Every configured resource bound to it then
-- fails capture with a missing path. The stable name of that tree is the
-- cache's `current` link, so point those bindings at it. Sources are otherwise
-- immutable provenance; the trigger is lifted only for this rewrite, and only
-- where it exists -- it is declarative state a 0.52 database has not seen yet.
DO $$
DECLARE
    guarded BOOLEAN;
BEGIN
    SELECT EXISTS (
        SELECT 1 FROM pg_trigger
        WHERE tgname = 'managed_sources_immutable'
          AND tgrelid = 'managed_sources'::regclass
    ) INTO guarded;
    IF guarded THEN
        ALTER TABLE managed_sources DISABLE TRIGGER managed_sources_immutable;
    END IF;

    UPDATE managed_sources
    SET specification = jsonb_set(
        specification,
        '{root}',
        to_jsonb(regexp_replace(specification->>'root', '/composed/[0-9a-f]{64}$', '/current'))
    )
    WHERE kind = 'local_tree'
      AND specification->>'root' ~ '/composed/[0-9a-f]{64}$';

    IF guarded THEN
        ALTER TABLE managed_sources ENABLE TRIGGER managed_sources_immutable;
    END IF;
END
$$;
