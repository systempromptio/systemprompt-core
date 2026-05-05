# systemprompt-database Tech Debt Audit

**Layer:** infra
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04 (Wave B3 compliance sweep)
**Re-validated:** 2026-05-05 (trait-surface typed-error cutover for 0.6.0)
**Verdict:** CLEAN

---

## Summary

| Category                                  | Before  | After |
|-------------------------------------------|---------|-------|
| `unwrap()` / `expect()`                   | 0       | 0     |
| `panic!()` / `todo!()` / `unimplemented!()` | 0     | 0     |
| `println!` / `eprintln!` / `dbg!`         | 0       | 0     |
| `let _ =` discards                        | 1       | 0 (justified `.ok()` only) |
| `.ok()` discards                          | 1       | 1 (justified) + 1 (env-var Result→Option idiom) |
| Inline `//` comments                      | 0       | 0     |
| `///` rustdoc on pub items                | 0       | **116 / 116** (100 %) |
| `//!` on pub modules                      | 0       | every `pub mod` covered |
| Files >300 lines                          | 1       | 0     |
| Raw `String` IDs                          | 0       | 0     |
| Raw `sqlx::query(_)` outside allowlist    | 29 (over-counted: real total **24**, **0** outside the documented allowlist paths) | 0 |
| `*Manager` suffix                         | 0       | 0     |
| `#[allow(...)]` (unjustified)             | 1       | 0 (the one remaining is documented) |
| `anyhow::` references in source           | 31      | 62 *(higher because docs-pass added context, not because new anyhow surface — see below)* |
| `async_trait` references                  | 9       | 9     |

`anyhow` count rose because rustdoc paragraphs reference `anyhow::Result` /
`anyhow::Error` / `RepositoryError` interchangeably to explain the boundary.
The actual `anyhow::Result`-returning **public** signatures are unchanged —
typed-error promotion was rolled back to keep the cross-crate surface
compatible (see "Anyhow handling" below).

---

## Anyhow handling (closed 2026-05-05)

The dyn-safe public surface (`DatabaseProvider`, `DatabaseTransaction`,
`DatabaseProviderExt`) and `FromDatabaseRow::from_postgres_row` now return
`DatabaseResult<T>` (alias for `Result<T, RepositoryError>`). The 2026-05-05
cutover (released as 0.6.0):

- Flipped 22 trait method signatures from `anyhow::Result<T>` to
  `DatabaseResult<T>` across `services/provider.rs`,
  `models/transaction.rs`, and `models/query.rs`.
- Added `RepositoryError::InvalidState(String)` plus
  `RepositoryError::invalid_state(msg)` to capture the driver-protocol
  invariant errors previously wrapped in `anyhow!` (transaction reused
  after commit, scalar query with no columns, unsupported `DbValue` type).
- Added a `From<RepositoryError> for systemprompt_traits::RepositoryError`
  bridge so domain repositories that hold the boxed-error variant pick up
  the typed error transparently through `?`.
- Extended `McpDomainError`, `OauthError`, `UserError`, `FilesError`, and
  `LoggingError` with `#[from] systemprompt_database::RepositoryError`
  variants so callers propagate via `?` without `.map_err`.
- Removed the legacy `From<anyhow::Error> for RepositoryError`,
  `From<anyhow::Error> for UserError`, and
  `From<anyhow::Error> for LoggingError` bridges — no longer load-bearing.

`anyhow::Result` no longer appears anywhere in the public signatures of
this crate.

---

## Documentation deltas

- `Cargo.toml`: added `[package.metadata.docs.rs]` with `all-features = true`.
- `lib.rs`: full crate-level `//!` doc with public-API map, feature-flag
  matrix (currently empty — documented as such), and sqlx allowlist statement.
- Every `pub` item now has a `///` doc comment.
- Every `pub mod` has a module-level `//!` doc comment.
- `RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-database --no-deps
  --all-features` is clean.

---

## File-split outcome

The single file >300 lines was `lifecycle/installation.rs` (320 lines). It
was split into a directory:

```
src/lifecycle/installation/
├── mod.rs        # re-exports
├── module.rs     # legacy on-disk Module install path
├── extension.rs  # inventory-registered Extension install path
└── util.rs       # shared `table_exists` helper
```

`repository/service.rs` (294 → 337 after rustdoc) was likewise split into
`repository/service/{mod,model,repo}.rs`. Final state: **0 files over 300
lines** (largest is now `repository/service/repo.rs` at 250).

---

## Per-sqlx-site decision table

All 24 truly-raw `sqlx::query(_)` call sites already lived inside the
documented allowlist directories (`src/admin/` and
`src/services/postgres/`). The allowlist files in `ci/check-sqlx.sh` cover
both directories recursively, so **(a) zero migrations** to verified macros
were performed, **(b) zero files were moved** between paths, and **(c) one
allowlist extension** was made: `services/postgres/mod.rs` and
`services/postgres/conversion.rs` were added to the per-file allowlist used
by `justfile` recipes `check-bans` / `check-bans-crate`.

| Site | Path | Decision | Reason |
|------|------|----------|--------|
| `services/postgres/ext.rs:15,35,52` | allowlist (existing) | keep dynamic | type-erased generic `T: FromDatabaseRow`; SQL is the runtime-supplied `&dyn QuerySelector` |
| `services/postgres/transaction.rs:39,61,83,105` | allowlist (existing) | keep dynamic | same reason — runtime SQL inside transactional context |
| `services/postgres/mod.rs:83,114,131,148,203,213,225,242` (8) | **allowlist (extended)** | keep dynamic | `DatabaseProvider` impl is the dynamic-SQL boundary; `mod.rs` was missing from the per-file allowlist used by `justfile` and is now added |
| `services/postgres/mod.rs:203` (`SELECT 1`) | allowlist (extended) | keep dynamic | connection probe; static SQL but lives next to the dynamic-SQL trait impl |
| `services/postgres/introspection.rs:8,13,26,29` (4) | allowlist (existing) | keep dynamic | `information_schema` walk; `count_query` is built per-table at runtime |
| `services/postgres/conversion.rs` | **allowlist (extended)** | keep dynamic | hosts the `bind_params(sqlx::query::Query, …)` helper consumed by all four files above |
| `admin/introspection.rs:21,58,70,113` (4) | allowlist (existing) | keep dynamic | admin describe-table / list-tables; SQL is built dynamically against runtime-supplied `SafeIdentifier` table names |
| `admin/query_executor.rs:58` | allowlist (existing) | keep dynamic | the entire purpose is to execute operator-supplied SQL parsed through `AdminSql` |

`grep -RInE 'sqlx::query\s*\(' crates/infra/database/src` after the change
returns **24** sites, all matched by the regex
`crates/infra/database/src/(admin/|services/postgres/(mod|introspection|query_executor|transaction|ext|conversion)\.rs)`.

`./ci/check-sqlx.sh` still passes against the broader workspace because its
allowlist is path-prefix based (`crates/infra/database/src/admin/`,
`crates/infra/database/src/services/postgres/`) and already covers every
file above.

---

## Allowlist-config touch points

- `justfile` — recipes `check-bans` and `check-bans-crate` updated to
  include `mod.rs` and `conversion.rs` in the per-file allowlist regex.
- `ci/check-sqlx.sh` — **no change required**; existing path-prefix entries
  already cover the entire `services/postgres/` directory.
- `instructions/prompt/rust.md` — file does not exist in this worktree
  snapshot, so no edit was made; the policy restated in `lib.rs`'s crate
  doc and in this audit document.

---

## Architectural compliance

Layer: `infra`. Dependencies flow downward only:
shared → infra/database. No circular dependencies. No upward dependency
introduced.

---

## Verification gate

Local commands all clean:

```text
cargo fmt -p systemprompt-database --check
cargo build -p systemprompt-database --all-features
cargo clippy -p systemprompt-database --all-targets --all-features -- -D warnings
RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-database --no-deps --all-features
just check-bans-crate systemprompt-database
just lint-sqlx
cargo build --workspace                # downstream cascade verified clean
```

---

## Verdict

**CLEAN**
