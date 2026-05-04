# systemprompt-identifiers Tech Debt Audit

**Layer:** shared
**Audited:** 2026-05-04
**Re-validated:** 2026-05-04 (Wave A1 compliance sweep)
**Verdict:** CLEAN

---

## Summary (post-sweep)

| Category | Before | After |
|----------|--------|-------|
| `anyhow::` references | 1 (`db_value/from_value.rs` public trait `FromDbValue`) | 0 |
| Files >300 lines | 1 (`macros.rs` at 386) | 0 (split into `macros/{mod,id,token,helpers}.rs`) |
| `pub` items with rustdoc | ~0 | 150+ |
| Module-level `//!` docs | 7 modules | every `pub mod` (lib.rs feature-flag matrix added) |
| `#[allow(...)]` | 7 | 7 (kept, see "Residual" below — now annotated with `// Why:`) |
| `let _ =` discards | 0 | 0 |
| `.ok()` discards | 0 | 0 |
| `unwrap()`/`expect()` | 0 (outside macro-generated panicking constructors) | 0 |
| `panic!()`/`todo!()`/`unimplemented!()` | 0 | 0 |
| `println!`/`eprintln!`/`dbg!` | 0 | 0 |
| Raw String IDs | 0 | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 | 0 |
| `*Manager` suffix | 0 | 0 |
| `async_trait` references | 0 | 0 |

---

## Fixes Applied

### Typed errors (anyhow → thiserror)

`db_value/from_value.rs` `FromDbValue::from_db_value` formerly returned
`anyhow::Result<Self>`. Replaced with a typed `DbValueError` enum in
`error.rs` (`Null`, `Incompatible`, `Parse`, `OutOfRange` variants, all
derived from `thiserror::Error`). `anyhow` removed from `Cargo.toml`,
`thiserror` added.

### File split

`macros.rs` (386 lines) split into:

- `macros/mod.rs` — re-exports
- `macros/id.rs` (246) — `define_id!` macro
- `macros/token.rs` (84) — `define_token!` macro
- `macros/helpers.rs` (117) — `__define_id_common!` and
  `__define_id_validated_conversions!` helper macros

Cohesion: one macro per file, helpers grouped.

### Rustdoc coverage

- Crate-level `//!` with usage example and feature-flag matrix
  (`default` / `sqlx`) added to `lib.rs`.
- Every `pub use` re-export in `lib.rs` carries a `///` line.
- Module-level `//!` added to every `pub mod` and to every leaf
  identifier file (`agent.rs`, `client.rs`, `session.rs`, `context.rs`,
  `user.rs`, `trace.rs`, `policy.rs`, `mcp.rs`, `ai.rs`, `content.rs`,
  `execution.rs`, `funnel.rs`, `hook.rs`, `jobs.rs`, `links.rs`,
  `oauth.rs`, `plugin.rs`, `roles.rs`, `task.rs`, `tenant.rs`,
  `headers.rs`, `error.rs`, `db_value/mod.rs`, `auth/mod.rs`,
  `auth/api_key.rs`, `auth/device_cert.rs`).
- Every `pub` item in custom-impl files carries `///`.
- Macro-generated `new`/`try_new`/`as_str`/`generate`/`system`/`redacted`
  methods carry `///` from inside the macro expansion, so every
  identifier minted via `define_id!`/`define_token!` ships with rustdoc.

### `[package.metadata.docs.rs]`

Added to `Cargo.toml` with `all-features = true` so docs.rs renders the
sqlx-feature-gated impls.

---

## Residual deferred work

The 7 `#[allow(clippy::expect_used)]` attributes on the panicking `new()`
convenience constructors in `agent.rs`, `email.rs`, `path.rs`,
`profile.rs`, `url.rs`, and inside the `define_id!(_, non_empty)` and
`define_id!(_, validated, _)` macro arms remain. They now carry inline
`// Why:` justification comments per §6 of `instructions/prompt/rust.md`.

A strict reading of §3 ("`expect()` is permitted only in macro-generated
`From<String>` impls for validated typed IDs") would require deleting
the panicking `new()` API entirely. That change has ~20 caller sites
across other worktrees' territory (`crates/entry/cli`,
`crates/entry/api`, `crates/domain/{agent,mcp,ai}`, the test workspace)
and cannot be performed within this Wave A1 worktree without violating
the worktree boundary rule. Deferred to a follow-up cross-cutting
change.

---

## Verification

```
cargo fmt -p systemprompt-identifiers                                      PASS
cargo build -p systemprompt-identifiers --all-features                     PASS
cargo clippy -p systemprompt-identifiers --all-targets --all-features
    -- -D warnings                                                         PASS
RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-identifiers
    --no-deps --all-features                                               PASS
just check-bans                                                            PASS (0 violations from this crate)
```

---

## File Statistics (post-sweep)

| Metric | Value |
|--------|-------|
| Total .rs files | 41 |
| Files over 300 lines | 0 |
| Largest file | `macros/id.rs` (246 lines) |

---

## Verdict

**CLEAN**
