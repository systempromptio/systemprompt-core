# systemprompt-api Audit — Area 6: server + proxy

Scope: `crates/entry/api/src/services/server/**` and `crates/entry/api/src/services/proxy/**`.
Entry binary crate — `anyhow` permitted, per-item `///` rustdoc banned.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | All deps flow downward (runtime/domain/infra/shared); no sideways or circular deps. |
| 2 | Error model | clean | `anyhow::Result` in entry code; `ProxyError` is a `thiserror` enum in `errors.rs`. |
| 3 | No panics | remediated | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`. Two silent `.unwrap_or()` on fallible `parse()` in `mcp_session.rs` now log a `tracing::warn!` before falling back. `client.rs` `unwrap_or_else` is a logged fallback, not a panic. |
| 4 | Raw SQL | clean | No `sqlx::query()`. `health.rs` uses the `DatabaseQuery`/`DatabaseProvider` abstraction, not raw SQLx; this is the established entry-crate API surface. |
| 5 | File size | clean | Largest file is `agents.rs` at 299 lines; all under the 300-line limit. |
| 6 | Function size | clean | All functions within ~75-line guidance; reconciliation/agents already extracted into cohesive helpers. |
| 7 | Async traits | clean | No trait definitions in scope; no `#[async_trait]`. |
| 8 | Typed identifiers | clean | `AgentId`/`AgentName`/`UserId` constructed via `::new`. Proxy `service_name` is a free-form HTTP path segment, not an entity ID. |
| 9 | Comment standard | clean | No per-item `///` in scope; inline `//` absent except none; `#[allow]` attributes justified by axum patterns. |
| 10 | No legacy | clean | No shims, dual paths, or `Option<T>` migration stubs. |
| 11 | Naming | clean | `*Service`/`*Handler`/`*Validator`/`*Builder`/`*Engine` used; no `*Manager` defined in scope (`McpManager`/`RegistryManager` are upstream crate types). |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | FATAL-message construction and event-send patterns are similar but context-specific; not worth a shared helper. |
| 14 | CHANGELOG | n/a | Not edited (observations only). |

## Remediation summary

- `proxy/engine/mcp_session.rs`: `enrich_with_cached_identity` previously discarded
  `UserType` and `Uuid` parse errors silently via `.unwrap_or(...)`. Both now log a
  `tracing::warn!` before defaulting (`UserType::Unknown` / `Uuid::nil()`), per the
  standard's rule against discarding `Result` without a warning.

No behavioural changes beyond added log lines.
