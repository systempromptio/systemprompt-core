# Audit remediation: `systemprompt-database`

Crate: `crates/infra/database/`. Audit wave: 2026-05 workspace audit.

| ID | Issue | Status | Note |
|----|-------|--------|------|
| A1 | `query_raw_with` broke the trait parameter convention (`Vec<serde_json::Value>`) | remediated | Already converted to `params: &[&dyn ToDbValue]` across `provider.rs`, `postgres/mod.rs` (`bind_params` path), `database.rs`, and all call sites; no `// JSON:` comments remain. |
| A2 | Migration bookkeeping built SQL by string interpolation | remediated | `record_migration` and `apply_squash_rows` already use parameterised `db.execute(...)` with bound `ToDbValue` params; no `.replace('\'', "''")` escaping remains. |
| A3 | `parse_sql_statements` exceeded the 75-line limit | remediated | Byte-state-machine extracted into a `Splitter` struct with per-`SplitState` step functions (`step_normal`, `step_single_quote`, `step_dollar_quote`, `step_line_comment`, `step_block_comment`); driver loop in `Splitter::run` is short. |
| A4 | CHANGELOG drift — false `sqlparser` claim | remediated | `[0.10.0]` `### Changed` entry already describes the hand-rolled byte-state-machine splitter and why (preserves verbatim text; parse-and-reprint drops the empty parameter list on `CREATE FUNCTION foo()`). `[0.9.2]` `parse_sql_statements` entry verified accurate. README.md file-table row corrected from "`sqlparser`-driven" to "hand-rolled byte-state-machine". |
| A5 | Functions marginally over 75 lines | remediated | `run_down_migrations`, `run_pending_migrations`, `exec.rs`, `schema_linter/columns.rs` all measured well under 75 lines; no split needed. |
| A6 | Whole-crate comment sweep | remediated | `services/`, `admin/`, `repository/`, `models/`, `services/postgres/` carry `//!` heads on every module; surviving `///` blocks all encode non-obvious constraints/invariants; zero WHAT-narration `//` comments. `database.rs` pool-extraction duplication collapsed into one private `require_postgres` helper used by `pool_arc`/`read_pool_arc`/`write_pool_arc`. |

## Verification

- `SQLX_OFFLINE=true cargo clippy -p systemprompt-database -p systemprompt-cli --all-targets --all-features -- -D warnings` — clean.
- `SQLX_OFFLINE=true cargo doc -p systemprompt-database --no-deps` — clean.
</content>
</invoke>
