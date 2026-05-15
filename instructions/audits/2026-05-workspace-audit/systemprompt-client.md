# Audit: systemprompt-client

Crate: `crates/shared/client/` — HTTP API client (shared layer). Audited 2026-05-15.

1. **Layering** — clean: depends only on `systemprompt-models` and `systemprompt-identifiers` (same shared layer) plus external crates; no upward/cross-layer deps.
2. **Error model** — clean: `thiserror`-derived `ClientError` enum in `error.rs`; no `anyhow` in public signatures.
3. **No panics** — clean: no `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!`; error-body read uses `unwrap_or_else` with a `tracing::warn!`.
4. **Raw SQL** — clean: crate contains no SQL.
5. **File size** — clean: largest file `client.rs` at 266 lines, under the 300-line limit.
6. **Function size** — clean: all functions well under 75 lines.
7. **Async traits** — clean: crate defines no traits.
8. **Typed identifiers** — clean: uses `ContextId`, `TaskId`, `JwtToken`; `agent_name: &str` and `base_url: String` are not entity IDs.
9. **Comment standard** — clean: substantive `//!` head on `lib.rs`; no `///` paraphrase noise, no narration comments.
10. **No legacy** — clean: no shims, dual paths, or migration stubs.
11. **Naming** — clean: `SystempromptClient`; no `*Manager`.
12. **Tests location** — clean: no inline `#[cfg(test)] mod tests`.
13. **Local duplication** — remediated: extracted `send_checked` in `http.rs` (replaced 4 repeated status-check blocks) and `SystempromptClient::limited_url` in `client.rs` (replaced 3 repeated optional-`?limit=` URL builders).
14. **CHANGELOG accuracy** — remediated: existing entries matched the code, but the latest entry was `0.9.2` while the workspace is at `0.10.1`; added a `0.10.1` version-consistency entry.
