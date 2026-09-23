-- A normalised snapshot of a database's `public` schema, one line per object,
-- so an upgraded database and a fresh install can be diffed line by line.
-- Used by scripts/schema-ladder.sh; run with `psql -XAtq -f`.
--
-- Names neither path controls are dropped: constraint names (a migration names
-- its constraint, a fresh install lets Postgres name it) and index names. A
-- UNIQUE constraint is compared through the unique index it owns. CHECK
-- expressions are compared with casts and grouping stripped, because Postgres
-- re-deparses the same expression with casts placed differently. Everything
-- else — column types, nullability, defaults, key definitions, index
-- definitions, views, triggers, function bodies, enum labels — is verbatim.
-- Functions a Postgres extension installed (pgcrypto, vector) are excluded:
-- they belong to the extension a release loaded, not to the schema.

SELECT format('column %s.%s %s%s%s', c.relname, a.attname,
              format_type(a.atttypid, a.atttypmod),
              CASE WHEN a.attnotnull THEN ' NOT NULL' ELSE '' END,
              COALESCE(' DEFAULT ' || pg_get_expr(d.adbin, d.adrelid), ''))
FROM pg_attribute a
JOIN pg_class c ON c.oid = a.attrelid
JOIN pg_namespace n ON n.oid = c.relnamespace
LEFT JOIN pg_attrdef d ON d.adrelid = a.attrelid AND d.adnum = a.attnum
WHERE n.nspname = 'public' AND c.relkind IN ('r', 'p') AND a.attnum > 0 AND NOT a.attisdropped
UNION ALL
SELECT format('constraint %s %s %s', c.conrelid::regclass, c.contype,
              CASE WHEN c.contype = 'c'
                   THEN regexp_replace(regexp_replace(pg_get_constraintdef(c.oid),
                            '::[a-z ]+(\[\])?', '', 'g'), '[()\[\]]', '', 'g')
                   ELSE pg_get_constraintdef(c.oid) END)
FROM pg_constraint c
JOIN pg_namespace n ON n.oid = c.connamespace
WHERE n.nspname = 'public' AND c.contype <> 'u'
UNION ALL
SELECT format('index %s %s', tablename,
              regexp_replace(indexdef, '^CREATE (UNIQUE )?INDEX \S+ ON ', 'CREATE \1INDEX ON '))
FROM pg_indexes
WHERE schemaname = 'public'
UNION ALL
SELECT format('view %s %s', c.relname, regexp_replace(pg_get_viewdef(c.oid, true), '\s+', ' ', 'g'))
FROM pg_class c
JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE n.nspname = 'public' AND c.relkind IN ('v', 'm')
UNION ALL
SELECT format('trigger %s', pg_get_triggerdef(t.oid))
FROM pg_trigger t
JOIN pg_class c ON c.oid = t.tgrelid
JOIN pg_namespace n ON n.oid = c.relnamespace
WHERE n.nspname = 'public' AND NOT t.tgisinternal
UNION ALL
SELECT format('function %s(%s) %s result=%s set=%s language=%s security_definer=%s volatility=%s parallel=%s strict=%s config=%s',
              p.proname, pg_get_function_identity_arguments(p.oid), md5(p.prosrc),
              pg_get_function_result(p.oid), p.proretset, l.lanname, p.prosecdef,
              p.provolatile, p.proparallel, p.proisstrict, p.proconfig)
FROM pg_proc p
JOIN pg_namespace n ON n.oid = p.pronamespace
JOIN pg_language l ON l.oid = p.prolang
WHERE n.nspname = 'public'
  AND NOT EXISTS (SELECT 1 FROM pg_depend d
                  WHERE d.classid = 'pg_proc'::regclass AND d.objid = p.oid AND d.deptype = 'e')
UNION ALL
SELECT format('enum %s %s', t.typname, string_agg(e.enumlabel, ',' ORDER BY e.enumsortorder))
FROM pg_type t
JOIN pg_enum e ON e.enumtypid = t.oid
JOIN pg_namespace n ON n.oid = t.typnamespace
WHERE n.nspname = 'public'
GROUP BY t.typname;
