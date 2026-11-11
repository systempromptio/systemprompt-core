//! Read-only digest of the live schema objects.
//!
//! The settled decision must describe the database, not only the binary: an
//! older binary booting during a rolling update (or after a rollback) re-runs
//! its own declarative DDL and never touches the fingerprint row. The digest
//! covers every table's columns, view, index, function, trigger and
//! constraint outside the system schemas and outside Postgres extensions, so
//! any definition changed or dropped since the last complete install sends the
//! next install down the full path. It reads `pg_catalog` only and takes no
//! locks a `SELECT` would not.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::LoaderError;
use systemprompt_identifiers::ExtensionId;

use crate::services::DatabaseProvider;

pub async fn catalog_digest(db: &dyn DatabaseProvider) -> Result<String, LoaderError> {
    sqlx::query_scalar!(
        r#"WITH ns AS (
    SELECT oid FROM pg_namespace
    WHERE nspname NOT IN ('pg_catalog', 'information_schema', 'pg_toast')
      AND nspname NOT LIKE 'pg_temp%' AND nspname NOT LIKE 'pg_toast_temp%'
), member AS (
    SELECT objid FROM pg_depend WHERE deptype = 'e'
), defs AS (
    SELECT 'r' AS kind, c.oid::regclass::text AS name,
           string_agg(a.attname || ' ' || format_type(a.atttypid, a.atttypmod)
                      || CASE WHEN a.attnotnull THEN ' NOT NULL' ELSE '' END
                      || coalesce(' DEFAULT ' || pg_get_expr(d.adbin, d.adrelid), ''),
                      ', ' ORDER BY a.attnum) AS def
    FROM pg_class c
    JOIN pg_attribute a ON a.attrelid = c.oid AND a.attnum > 0 AND NOT a.attisdropped
    LEFT JOIN pg_attrdef d ON d.adrelid = c.oid AND d.adnum = a.attnum
    WHERE c.relnamespace IN (SELECT oid FROM ns) AND c.relkind IN ('r', 'p')
      AND c.oid NOT IN (SELECT objid FROM member)
    GROUP BY c.oid
    UNION ALL
    SELECT 'v', c.oid::regclass::text, pg_get_viewdef(c.oid)
    FROM pg_class c
    WHERE c.relnamespace IN (SELECT oid FROM ns) AND c.relkind IN ('v', 'm')
      AND c.oid NOT IN (SELECT objid FROM member)
    UNION ALL
    SELECT 'i', c.oid::regclass::text, pg_get_indexdef(c.oid)
    FROM pg_class c
    WHERE c.relnamespace IN (SELECT oid FROM ns) AND c.relkind IN ('i', 'I')
      AND c.oid NOT IN (SELECT objid FROM member)
    UNION ALL
    SELECT 'f', p.oid::regprocedure::text, pg_get_functiondef(p.oid)
    FROM pg_proc p
    WHERE p.pronamespace IN (SELECT oid FROM ns) AND p.prokind IN ('f', 'p')
      AND p.oid NOT IN (SELECT objid FROM member)
    UNION ALL
    SELECT 't', t.tgrelid::regclass::text || '.' || t.tgname, pg_get_triggerdef(t.oid)
    FROM pg_trigger t
    WHERE NOT t.tgisinternal
      AND t.tgrelid IN (SELECT oid FROM pg_class WHERE relnamespace IN (SELECT oid FROM ns))
    UNION ALL
    SELECT 'c', con.conrelid::regclass::text || '.' || con.conname, pg_get_constraintdef(con.oid)
    FROM pg_constraint con
    WHERE con.connamespace IN (SELECT oid FROM ns) AND con.conrelid <> 0
)
SELECT md5(coalesce(
    string_agg(kind || ':' || name || '=' || def, E'\n' ORDER BY kind, name, def), ''
)) AS "digest!"
FROM defs"#
    )
    .fetch_one(db.get_postgres_pool().as_ref())
    .await
    .map_err(|e| LoaderError::SchemaInstallationStepFailed {
        extension: ExtensionId::new("database"),
        context: "Failed to digest the live schema catalog".to_owned(),
        source: Box::new(e),
    })
}
