# Audit — `systemprompt-events` (`crates/infra/events/`)

Date: 2026-05-15. Scope: 14-item workspace audit checklist.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Deps are external + `systemprompt-identifiers`/`systemprompt-models` (shared); no upward/cross-layer deps. |
| 2 | Error model | clean | `EventError` is a `thiserror` enum in `error.rs`; no `anyhow` in public signatures. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`; serialization failure logs via `tracing` and returns 0. |
| 4 | Raw SQL | clean | Crate touches no database. |
| 5 | File size | clean | Largest file `broadcaster.rs` at 200 lines, under the 300 limit. |
| 6 | Function size | clean | All functions well under 75 lines. |
| 7 | Async traits | remediated | `Broadcaster` used `#[async_trait]` with no `dyn` requirement and no documented reason — converted to native `async fn` in trait. |
| 8 | Typed identifiers | remediated | `Broadcaster::register`/`unregister` took `connection_id: &str` — changed to `&ConnectionId`; call sites in `entry/api` updated. Internal `HashMap` string keys are an implementation detail and left as-is. |
| 9 | Comment standard | clean | Substantive `//!` heads; no paraphrase `///`; no narration comments. |
| 10 | No legacy | clean | No shims, dual paths, or `Option<T>` stubs. |
| 11 | Naming | clean | `EventRouter`, `*Broadcaster`, `ConnectionGuard` — no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | `ToSse` impls share `serialize_to_sse`; stale-connection cleanup is small and contextually distinct. |
| 14 | CHANGELOG accuracy | remediated | 0.9.2 entry described `EventError`/`EventResult` as covering "fallible broadcaster operations" — no broadcaster operation returns `EventResult` (`broadcast` returns `usize`). Reworded to reflect actual surface. |

## Remediation summary

- Converted `Broadcaster` trait from `#[async_trait]` to native `async fn` in trait (and the `GenericBroadcaster` impl). The trait has an associated type and is never used as `dyn`, so `#[async_trait]` was unjustified overhead.
- Typed the `connection_id` parameter of `Broadcaster::register`/`unregister` as `&ConnectionId` instead of `&str`; updated the two `entry/api` call sites to pass `&conn_id`.
- Corrected the 0.9.2 CHANGELOG entry: `EventError`/`EventResult` exist as the crate's public error surface but are not yet wired into any fallible broadcaster operation.
