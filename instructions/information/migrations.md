# Database schema and migrations

This document defines how schema changes reach a running systemprompt deployment, and the invariants the runtime enforces.

## The rule

**Declarative schema and imperative migration are physically separate.**

- `<crate>/schema/<name>.sql` is **pure declarative target state**. It describes what the schema looks like, not how to get there from any prior shape. Only idempotent `CREATE` statements live here. The runner lints every schema file at boot and **hard-rejects** anything imperative.
- `<crate>/schema/migrations/NNN_<name>.sql` is **versioned imperative state transition**. ALTER, DROP, DO `$$ … $$`, INSERT/UPDATE/DELETE seeds — all the operations that take a database from shape A to shape B — live here. Each migration runs once per `(extension_id, version)` and is checksummed in `extension_migrations`.

The runner applies migrations FIRST, then executes the schema. Migrations bring legacy tables up to current shape; the schema then sees a target-state-compliant table and every `CREATE … IF NOT EXISTS` is a clean no-op.

This is the same separation Diesel, Alembic, Flyway, and `sqlx migrate` converged on, for the same reason: mixing them produces `column "x" does not exist` failures on legacy databases when a schema's `CREATE INDEX … ON table(col)` runs against a `CREATE TABLE IF NOT EXISTS` that was a no-op against an older shape.

## What's allowed in schema files

| Allowed | Notes |
|---------|-------|
| `CREATE TABLE IF NOT EXISTS …` | Idempotent. `IF NOT EXISTS` is required; the linter warns otherwise. |
| `CREATE [UNIQUE] INDEX IF NOT EXISTS …` | |
| `CREATE [OR REPLACE] FUNCTION …` | |
| `CREATE [OR REPLACE] VIEW …` | If the view's column set must change, the change is structural — move to a migration. |
| `CREATE [OR REPLACE] TRIGGER …` | Postgres 14+. |
| `CREATE TYPE …` | |
| `CREATE EXTENSION IF NOT EXISTS …` | |
| `COMMENT ON …` | |

## What's rejected in schema files

`ALTER TABLE`, `DROP TABLE`, `DROP INDEX`, `DROP COLUMN`, `DROP VIEW`, `DROP TRIGGER`, `DROP CONSTRAINT`, top-level `DO $$ … $$`, `UPDATE`, `INSERT`, `DELETE`, `TRUNCATE`, `GRANT`, `REVOKE`. The linter (`crates/infra/database/src/services/schema_linter.rs`) fails the install with a line/column pointer at the offending statement.

The pre-merge scanner `./ci/lint-schema.sh crates` runs the same checks via grep; it is wired into `just check`.

## How an extension declares migrations

Migrations are discovered from the filesystem, not hand-listed in Rust. A crate
that has migrations is wired once:

- a one-line `build.rs` at the crate root:

  ```rust
  fn main() {
      systemprompt_extension::build::emit_migrations();
  }
  ```

- `systemprompt-extension` as a `[build-dependencies]` entry, and `build.rs`
  added to the package `include` list so it ships on `cargo publish`;
- `Extension::migrations()` returns the `extension_migrations!()` macro:

  ```rust
  fn migrations(&self) -> Vec<Migration> {
      extension_migrations!()
  }
  ```

At build time the script scans `<crate>/schema/migrations/`, derives each
migration's version (`NNN`) and name from the filename, and generates the
`Vec<Migration>` body. Because the filename is the single source of version and
name, those values cannot drift from the SQL they label. `cargo:rerun-if-changed`
makes a newly added file retrigger the build. Inline migration SQL as a Rust
string constant, or a hand-written `Migration::new(...)` list, is rejected by
`just lint-extensions`.

A migration whose first non-blank line is `-- @no-transaction` is run outside a
transaction (for `CREATE INDEX CONCURRENTLY`). A paired
`<crate>/schema/migrations/NNN_<name>.down.sql` supplies the down migration.

## Adding a column to an existing table

1. Update the declarative `<crate>/schema/<table>.sql` so the `CREATE TABLE IF NOT EXISTS` block includes the new column. Fresh installs pick it up directly.
2. Add a migration file `<crate>/schema/migrations/NNN_<name>.sql` — the next number after the current highest:

   ```sql
   ALTER TABLE markdown_content
       ADD COLUMN IF NOT EXISTS locale TEXT NOT NULL DEFAULT 'en';
   ```

   That is the whole change. The build script picks the file up; there is no Rust list to edit.
3. Optionally adjust matching indexes in the schema. The migration runs first; the schema's `CREATE INDEX IF NOT EXISTS` then succeeds against the new column.

## Renaming or dropping a column

Pure migration territory. The declarative schema describes the post-rename / post-drop shape. The migration moves the existing data.

```sql
ALTER TABLE markdown_content RENAME COLUMN factura TO numero_factura;
```

There is no way to do this in a schema file — and that's the point. Anything destructive needs a versioned record.

## View rewrites

If the view's column set stays the same, `CREATE OR REPLACE VIEW` works in the schema and is the right move. If the column set changes (column dropped, type changed, name changed), Postgres rejects `CREATE OR REPLACE`. In that case the change is structural: put both the `DROP VIEW … CASCADE` and the new `CREATE VIEW` in a migration. The declarative schema then carries only the new view definition.

## Seed data

INSERT statements are rejected in schemas. If an extension needs default rows, write a migration that inserts with `ON CONFLICT (…) DO NOTHING`:

```sql
INSERT INTO anomaly_thresholds (metric_name, warning_threshold, critical_threshold)
VALUES ('latency_ms', 100, 500)
ON CONFLICT (metric_name) DO NOTHING;
```

## Install order

For each extension, sorted by `migration_weight()`:

1. `run_pending_migrations(ext)` — apply every migration whose `(ext_id, version)` is not yet recorded in `extension_migrations`.
2. `install_extension_schema(ext)` — lint, parse, execute each `SchemaDefinition.sql` inside a single transaction.

A session-scoped `pg_advisory_lock(0x73_70_72_6F_6D_70_74_01)` serialises concurrent boots so rolling deploys cannot race on DDL.

## Migration checksums

Each migration's SQL is hashed at apply time and stored in `extension_migrations`. Edits after apply are rejected unless `--allow-checksum-drift` is passed. The drift flag does not re-apply the migration; it acknowledges and proceeds.

## Dependencies and weight ordering

`Extension::migration_weight()` controls install order. An extension's declared `dependencies()` must each have a strictly lower weight; the registry refuses to start otherwise (`LoaderError::InvalidDependencyOrdering`).

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

## Auditing live drift

`systemprompt infra db doctor` reports tables that exist in the live database but are not declared by any registered extension, and `SchemaDefinition.required_columns` entries that are declared but missing.

## Common failure modes

| Symptom | Likely cause | Fix |
|---------|--------------|-----|
| `imperative SQL in declarative schema: ALTER …` at install | A schema file contains a state transition | Move to `schema/migrations/NNN_<name>.sql`, declare via `migrations()`. |
| `column "X" does not exist` during `CREATE INDEX` | Legacy table predates the column; no migration adds it | Add a migration `ALTER TABLE … ADD COLUMN IF NOT EXISTS X …`. |
| `Migration N has been edited since it was applied` | Migration SQL changed after apply | Add a new migration version, or pass `--allow-checksum-drift` if intentional. |
| `Required column 'X' not found in table 'Y'` | The schema declares `with_required_columns` but the live table is missing it after install | Live schema diverged; `db doctor` to inspect, write the migration that brings the live table up to current. |
