# Audit — systemprompt-cli Area 7: plugins / shared / commands root

Scope: `crates/entry/cli/src/commands/plugins/**`, `crates/entry/cli/src/commands/shared/**`,
and root `.rs` files directly in `crates/entry/cli/src/commands/` (`mod.rs`).

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Depends only on lower layers (extension/loader/mcp/runtime/models); no sideways or circular deps. |
| 2 | Error model | clean | Entry crate; `anyhow::Result` + `Context` used throughout, permitted here. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`; only `unwrap_or`/`unwrap_or_else` fallbacks. |
| 4 | Raw SQL | clean | No SQL in scope; DB access delegated to repository/service types. |
| 5 | File size | clean | Largest file `mcp/validate.rs` at 240 lines; all under the 300-line limit. |
| 6 | Function size | clean | All functions within ~75-line guidance; longer ones already extracted into sub-fns. |
| 7 | Async traits | clean | No trait definitions in scope; plain `async fn` only. |
| 8 | Typed identifiers | clean | `PluginId`/`SessionToken` used; remaining `String` server/tool names match external MCP config keys. |
| 9 | Comment standard | clean | No `///` per-item docs in scope; `//!` heads on `mcp/mod.rs`, `mcp/list_packages.rs` are substantive. |
| 10 | No legacy | remediated | Removed dead `let _manager = McpManager::new(...)?` binding (and its unused import) in `mcp/validate.rs`. |
| 11 | Naming | clean | `*Service`/`*Args`/`*Output`; no `*Manager` types defined in scope (external `McpManager`/`DatabaseManager` out of scope). |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | remediated | Extracted repeated `ExtensionRegistry::discover().unwrap_or_else(...)` (9 sites) into private `discover_registry()` in `plugins/mod.rs`. |
| 14 | CHANGELOG | clean | Observations only; `CHANGELOG.md` not modified. |
