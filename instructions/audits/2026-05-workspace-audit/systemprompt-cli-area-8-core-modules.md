# Audit — systemprompt-cli (Area 8: core modules)

Scope: `crates/entry/cli/src/` excluding `src/commands/`. Entry binary crate (`anyhow` permitted, per-item `///` banned).

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Only downward deps (config/cloud/database/agent/logging/runtime/identifiers/models); no sideways or circular deps. |
| 2 | Error model | clean | `anyhow::Result` throughout — permitted in entry crate. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`todo!`/`dbg!` in scope. |
| 4 | Raw SQL | clean | No `sqlx::query` calls; DB access goes through `ContextRepository`. |
| 5 | File size | clean | Largest in-scope file is `args.rs` at 278 lines, under the 300 limit. |
| 6 | Function size | clean | All functions within ~75-line guidance; orchestration split across `mod.rs`/`helpers.rs`. |
| 7 | Async traits | clean | No trait definitions / no `#[async_trait]` in scope. |
| 8 | Typed identifiers | remediated | `routing::ExecutionTarget::Remote.token` changed from raw `String` to `SessionToken`; call site updated. |
| 9 | Comment standard | clean | No per-item `///` in scope; `//!` heads concise; no WHAT-comments. Added one `// JSON:` justification. |
| 10 | No legacy | clean | No shims, dual paths, `Option<T>` stubs, or dead code. |
| 11 | Naming | clean | No `*Manager`; `*Service`/`*Handler` usage consistent. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | No copy-paste blocks warranting extraction. |
| 14 | CHANGELOG accuracy | remediated | 0.9.2 entry described `init_credentials_gracefully` matching `CredentialsFileNotFound` directly; commit 6f74df0e broadened it to `is_local_mode_recoverable()` without a note. Added a 0.10.2 entry recording actual behaviour. |

## Remediation detail

- `session/resolution/helpers.rs`: two silent `.ok()?` `Result` discards (`ContextRepository::new`, `create_context`) now log via `tracing::debug!`/`warn!` before conversion.
- `shared/profile.rs`: `build_discovered_profile` now `tracing::warn!`s before discarding an unreadable profile during discovery.
- `shared/command_result.rs`: added `// JSON:` justification on the `serde_json::Value` renderer-hint bag (open-ended, cannot be typed).
- `routing/mod.rs` + `lib.rs`: `ExecutionTarget::Remote.token` is now `SessionToken`; `execute_remote` boundary keeps `&str` (opaque bearer transport, not an ID lookup).

No behavioural changes; all edits are standards compliance and a typed-ID tightening.
