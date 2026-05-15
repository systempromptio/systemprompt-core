# systemprompt-generator Tech Debt Audit

**Layer:** app
**Audited:** 2026-05-04 (refreshed during Wave D3 sweep)
**Verdict:** CLEAN

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 0 |
| `.ok()` discards | 0 |
| Inline `//` comments | 0 |
| Doc `///` comments on every public item | YES |
| Files >300 lines | 0 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 0 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references in public signatures | 0 |
| `async_trait` references | 8 (acceptable — used for `dyn` provider/job traits) |

---

## Wave D3 Changes

- Added typed `PublishError` variants (`Io`, `Yaml`, `Json`, `Other`) and a
  canonical `GeneratorResult<T>` alias re-exported from `lib.rs`.
- Removed all `anyhow::Result` / `anyhow::Error` from public signatures and
  internal helpers; remaining `anyhow` usage in the crate is zero.
- Split `prerender/content.rs` (322 LOC) into `content.rs` (orchestration)
  and `render.rs` (per-item rendering), both <200 LOC.
- Moved `error.rs` → `error/mod.rs` and extracted suggestion heuristics into
  `error/suggestions.rs` to keep the error module under 300 LOC.
- Replaced the bare `.ok()?` discard in `build/validation.rs` with a typed
  branch that logs at WARN before skipping the unparseable URL.
- Added module-level `//!` docs to every `pub mod` and `///` rustdoc to every
  remaining public item (XML helpers, ToC types, build orchestrator,
  templates, jobs, prerender entry points, error variants).
- Added `[package.metadata.docs.rs] all-features = true` to `Cargo.toml`.
- `lib.rs` now ships a feature-flag matrix and a public-surface tour at the
  top.

## Verdict

**CLEAN**
