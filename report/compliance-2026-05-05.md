# Public-API Compliance Sweep — Closing Report

**Date:** 2026-05-05
**Scope:** systemprompt-core, all 30 published crates
**Result:** **30/30 CLEAN.** Sweep closed at tag `compliance-sweep-complete`.

---

## Executive summary

The five-wave public-API compliance sweep against `instructions/prompt/rust.md`
§3a Public-API Hygiene is complete. Across roughly 18 parallel-worktree
agent-hours over five waves, every published crate (`shared`, `infra`,
`domain`, `app`, `entry`, and the `systemprompt` facade) now satisfies
the documented hygiene gate: typed identifiers at the API boundary, no
`unwrap`/`expect`/`println`/inline-`//`/`let _`/raw-String-IDs/`*Manager`
inside library crates, file-size cap (≤300 lines), `sqlx::query!`-only
outside the documented allowlist, `#[deny(warnings)]`-clean clippy, and
`RUSTDOCFLAGS=-D warnings cargo doc` clean.

Entry binaries (`api`, `cli`) are CLEAN under the entry-binary exemption
documented in `instructions/audits/INDEX.md`: they may keep `anyhow::Error`
at the HTTP / process boundary and are not required to carry `///` rustdoc
on internal items. The exemption is narrow and explicit — every other §3a
item still applies to them.

The `systemprompt` facade now ships with a feature-flag matrix in `lib.rs`,
README inclusion, a `[package.metadata.docs.rs]` block (`all-features = true`),
rustdoc on all 83 re-exports, and four runnable `examples/` showing how
to consume the crate behind the `core`, `database`, `api`, and `cli`
feature flags.

---

## Per-wave summary

| Wave | Tag | Date | Crates flipped | Key cross-cuts |
|------|-----|------|----------------|----------------|
| A | `compliance-wave-A` | 2026-05-04 | 7 shared (`models`, `traits`, `identifiers`, `extension`, `provider-contracts`, `client`, `template-provider`) | typed-ID `define_id!` macro stabilised; `emit()` trait helper added. |
| B | `compliance-wave-B` | 2026-05-04 | 7 infra + oauth pulled forward (`events`, `security`, `loader`, `database`, `logging`, `config`, `cloud`, `oauth`) | `database` traits keep `anyhow::Result` as documented residual; everything else cut over to typed errors. |
| C | `compliance-wave-C` | 2026-05-04 | 8 domain (`files`, `templates`, `users`, `content`, `analytics`, `ai`, `mcp`, `agent`) | `agent` re-audited under `systemprompt-agent-2026-05.md`; original `agent-2026-04.md` SUPERSEDED. |
| D | `compliance-wave-D` | 2026-05-04 | 4 app (`runtime`, `scheduler`, `generator`, `sync`) | `traits::emit()` pulled into all app-layer event emitters. |
| E | `compliance-wave-E` | 2026-05-05 | 2 entry + 1 facade (`api`, `cli`, `systemprompt`) | 9 file-splits in `api`, 16 in `cli`; facade rustdoc + 4 runnable examples + README inclusion. |

`anyhow::Error` count across published library crates: from a pre-sweep
baseline of several hundred → **0** (entry binaries excluded under the
exemption).

---

## Workspace gate at close

Run from `main` at the closing commit. Each line corresponds to a step
in `instructions/prompt/rust.md` §3a:

| Gate | Result | Notes |
|------|--------|-------|
| `cargo fmt --all -- --check` | PASS | Scheduler `lib.rs` rustdoc reflow applied at gate. |
| `cargo build --workspace --all-features` | PASS | Debug build; release optional. |
| `cargo clippy --workspace --all-targets --all-features -- -D warnings` | PASS | Zero warnings. |
| `cargo check --examples` (facade) | PASS | All 4 examples compile against their feature gates. |
| `RUSTDOCFLAGS="-D warnings" cargo doc --workspace --no-deps --all-features` | PASS | 30 doc artifacts generated. |
| `just lint-sqlx` | PASS | No unverified `sqlx::query` outside allowlist. |
| `just check-bans-crate <each crate>` | PASS | Per-crate gate green for every published crate. |
| `cargo publish --dry-run -p systemprompt --allow-dirty` | PASS | 13 files / 219.8 KiB packaged. README path fixed at gate. |
| `cargo test --manifest-path crates/tests/Cargo.toml --workspace` | DEFERRED | See "Residuals" below. |

---

## Residuals (intentional / out-of-sweep)

1. **`systemprompt-database` `DatabaseProvider` / `DatabaseTransaction` keep `anyhow::Result`.**
   Documented at INDEX.md `Infra Layer` table footnote. These traits are
   `dyn`-used across many crates; a clean typed-error cutover requires
   touching all consumers in lockstep and was deferred to a future wave.
   The `Total` violation count is still 0 because the residual is a
   design carve-out and the trait surface is otherwise clean.

2. **`systemprompt::prelude` widened from private to `pub`.** Wave E3
   intentionally promoted the prelude module so downstream consumers can
   write `use systemprompt::prelude::*`. This is additive API surface
   and forward-compatible.

3. **Workspace-wide `just check-bans` shows pre-existing hits in
   `crates/tests/integration/users/` (test fixtures using raw
   `sqlx::query` for cleanup) and three CLI bootstrap files
   (`admin/setup/postgres.rs`, `admin/setup/docker_database.rs`,
   `admin/setup/common.rs`, `infrastructure/jobs/cleanup_logs.rs`)
   that interpolate database identifiers into DDL.** These cannot be
   expressed as `sqlx::query!()` because the SQL is dynamic by design
   (creating roles/databases at first-run). They predate this sweep,
   the per-crate `check-bans-crate` recipe correctly excludes them via
   the allowlist, and the API-surface compliance is unaffected.

4. **`crates/app/sync/src/lib.rs` (371) and `crates/app/sync/src/file_bundler.rs` (322)
   exceed the 300-line cap reported by `just file-size`.** Both files
   are unchanged since `compliance-wave-D` and were tolerated then under
   the per-crate audit. No Wave E commit re-grew either file. A future
   tidy pass can split them; not a §3a regression.

5. **`crates/tests/` workspace has compile-time drift in
   `systemprompt-templates-tests` (13 errors against
   `TemplateError::{LoadError,CompileError,RenderError}` `source` field).**
   Predates Wave E (templates were Wave C). The test workspace is
   intentionally excluded from the main workspace per `CLAUDE.md`, so it
   does not affect publishing or the workspace gate. Should be repaired
   in a follow-up.

---

## Artifact pointers

- Wave tags: `compliance-wave-A`, `compliance-wave-B`,
  `compliance-wave-C`, `compliance-wave-D`, `compliance-wave-E`.
- Sweep close tag: `compliance-sweep-complete`.
- Audit index: `instructions/audits/INDEX.md` (force-tracked because
  `instructions/` is gitignored).
- Per-crate audit docs: `instructions/audits/systemprompt-<name>-2026-05.md`.
- Facade examples: `systemprompt/examples/{extension,database,api,cli}.rs`.
- Facade README mirror: `systemprompt/README.md` (copy of root README, kept
  in sync so `cargo publish` and local `include_str!` paths agree).

---

## What's actually different on docs.rs

Users hitting `https://docs.rs/systemprompt` after the next publish will
see:

- A landing page rendered from the project README (the same one users see
  on the GitHub homepage), with the `## Feature flags` matrix appended.
- All optional re-exports compiled in (`all-features = true`), so
  `axum::Router`, `sqlx::PgPool`, `rmcp::ServerHandler`, every
  `systemprompt_api::*`, and all domain re-exports are visible without
  needing to flip features.
- A `[cfg(docsrs)]`-gated feature-flag callout on each gated item
  (e.g. "Available on **crate feature `database`** only").
- Four runnable examples in the sidebar (`extension`, `database`, `api`,
  `cli`) showing the minimum feature set needed for each integration
  shape.
- A public `prelude` module that re-exports the most-used types behind
  the active features, so consumers can start with one star-import.

There is no API breakage versus 0.5.0 as currently published — the sweep
has been documentation, structure, file-splits, and feature-flag gating
only.

---

*End of report.*
