# systemprompt-sync Tech Debt Audit

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
| Raw `sqlx::query` (outside allowlist) | 0 (audit false-positive — every site uses `sqlx::query!` macros) |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 0 |
| `anyhow::` references in public signatures | 0 |
| `async_trait` references | 2 (acceptable — used for the `Job` trait impl) |

---

## Wave D3 Changes

- Extended `SyncError` with `Yaml`, `InvalidInput`, and `Other` variants and
  re-exported the existing `SyncResult` alias from `lib.rs` with full
  rustdoc.
- Removed all `anyhow::Result` / `anyhow::Error` / `anyhow::Context` /
  `anyhow::bail!` from the crate. Upstream errors from the agent / content
  domain repositories are mapped via `SyncError::other` at the call site;
  YAML / I/O / SQLx / reqwest errors are auto-converted via `#[from]`.
- Mapped scheduled-job upstream failures into typed `ProviderError`
  variants (`Configuration`, `InvalidInput`, `RenderFailed`).
- Split `api_client.rs` (302 LOC) into `api_client/mod.rs`,
  `api_client/retry.rs` (`RetryConfig`), and `api_client/response.rs`
  (JSON / binary response handlers).
- Added module-level `//!` docs to every `pub mod` and `///` rustdoc to
  every public type, field, and method (lib.rs surface, models, files,
  file_bundler, database, diff, export, jobs, local, crate_deploy,
  api_client).
- Verified the audit's "3 raw `sqlx::query`" finding was a false-positive:
  every site already uses the compile-time-checked `sqlx::query!` macro.
- Removed the `anyhow` workspace dependency from `Cargo.toml`.
- Added `[package.metadata.docs.rs] all-features = true` to `Cargo.toml`.

## Verdict

**CLEAN**
