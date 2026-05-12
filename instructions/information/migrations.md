# Database schema and migrations

This document defines how schema changes reach a running systemprompt deployment, and the invariants the runtime enforces.

## TL;DR

- Each extension declares schemas via `SchemaDefinition` and (optionally) versioned `Migration` records.
- On boot (`systemprompt serve`) or `systemprompt infra db migrate`, every extension's schema SQL is re-executed under a transaction, and any pending versioned migrations are applied.
- Schema SQL must be idempotent (`CREATE TABLE IF NOT EXISTS`, `ADD COLUMN IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`). The runtime **always** executes it; there is no skip-if-table-exists shortcut.
- A session-scoped Postgres advisory lock serialises concurrent boots so rolling deploys cannot race on DDL.
- Versioned migrations record a checksum; edits after apply are an error unless `--allow-checksum-drift` is passed.

## Two mechanisms

1. **`SchemaDefinition`** — the file under `<crate>/schema/<name>.sql`, embedded via `include_str!` in `extension.rs`. Re-run every boot. Use for tables, indexes, constraints, additive column changes.
2. **`Migration`** — versioned records returned from `Extension::migrations()`. Run once per `(extension_id, version)`. Use for destructive changes (DROP COLUMN, data migrations, type changes) where you do not want a second boot to undo or re-apply the operation.

Both run inside `install_extension_schemas_full`, in this order, per extension, sorted by `migration_weight()`.

## Shipping an additive change

Add the new column to the existing `<crate>/schema/<table>.sql` with an `IF NOT EXISTS` guard, plus any matching index updates:

```sql
ALTER TABLE markdown_content
    ADD COLUMN IF NOT EXISTS locale TEXT NOT NULL DEFAULT 'en';

DROP INDEX IF EXISTS idx_markdown_content_slug;
CREATE UNIQUE INDEX IF NOT EXISTS idx_markdown_content_slug_locale
    ON markdown_content(slug, locale);
```

Then update the in-memory model and queries. On next deploy, every tenant — new or pre-existing — picks up the change.

## Shipping a destructive change

Add a `Migration` to the extension. Example:

```rust
fn migrations(&self) -> Vec<Migration> {
    vec![
        Migration::new(1, "drop_legacy_status_column", include_str!("../migrations/001_drop_legacy_status.sql")),
    ]
}
```

Once applied to any environment, the SQL content is frozen — its checksum is stored in `extension_migrations`. To change behaviour, add a **new** migration with a higher version.

If you must edit an applied migration (rare), invoke with `--allow-checksum-drift`. The runtime will log the mismatch and continue but will not re-apply the migration; you are responsible for reconciling the live schema manually.

## Dependencies and ordering

`Extension::migration_weight()` controls install order. An extension's declared `dependencies()` must each have a strictly lower weight; the registry refuses to start otherwise (`LoaderError::InvalidDependencyOrdering`). Current weights:

| Extension | Weight |
|-----------|--------|
| database  | 10     |
| users     | 100    |
| analytics | 200    |
| mcp       | 250    |
| oauth     | 300    |
| ai        | 350    |
| agent     | 400    |
| content   | 450    |
| files     | 500    |

## Advisory lock

`install_extension_schemas_full` acquires `pg_advisory_lock(0x73_70_72_6F_6D_70_74_01)` for the duration of the install pass. The lock is session-scoped: if a process exits mid-install, Postgres reclaims the lock automatically. Two `serve` invocations against the same database will serialise on this lock rather than race.

## Schema installation in `AppContext`

```rust
let ctx = AppContext::builder()
    .with_migrations(true)                       // install on build
    .with_migration_config(MigrationConfig {
        allow_checksum_drift: false,
    })
    .build()
    .await?;
```

`with_migrations(true)` is the canonical way to guarantee the schema matches the code. `systemprompt serve` sets it; admin tools that only want a read-only handle (e.g. `db doctor`) leave it off.

## Auditing live drift

`systemprompt infra db doctor` reports tables that exist in the live database but are not declared by any registered extension, and `SchemaDefinition.required_columns` entries that are declared but missing.

## Common failure modes

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| `Statement N/M failed: column "X" does not exist` during install | A query in the same SchemaDefinition references a column an earlier statement was supposed to add, but the earlier statement was skipped or failed | Inspect the failing statement in the error message; re-order statements within the file |
| `LoaderError::InvalidDependencyOrdering` at startup | Two extensions have equal or inverted `migration_weight()` | Adjust weights so each dep is strictly lower than its dependent |
| `Migration N has been edited since it was applied` | Migration SQL changed after apply | Add a new migration version, or pass `--allow-checksum-drift` if intentional |
| `Required column 'X' not found in table 'Y'` | The schema file declares `with_required_columns` but the live table is missing it after install | Live schema diverged from the file; run `db doctor` to inspect, then either restore the column manually or add an `ALTER TABLE … ADD COLUMN IF NOT EXISTS X` to the schema file |
