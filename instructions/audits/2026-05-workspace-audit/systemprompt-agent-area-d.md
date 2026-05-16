# Audit — `systemprompt-agent` Area D

Scope: `src/services/{shared,skills,registry}/`, root `src/services/*.rs`, `src/models/`, and crate-root `lib.rs`/`error.rs`/`extension.rs`/`state.rs`.

1. **Layering** — clean. Dependencies stay within shared/infra plus declared sibling domain crates.
2. **Error model** — clean. `AgentError`/`AgentServiceError` are `thiserror`-derived; no `anyhow` in public signatures.
3. **No panics** — clean. No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!` in scope.
4. **Raw SQL** — clean. No `sqlx::query*` in service/model files; all SQL lives in repositories (out of scope).
5. **File size** — clean. Largest in-scope file is 254 lines, under the 300 limit.
6. **Function size** — clean. No function exceeds the 75-line guidance.
7. **Async traits** — clean. The two `#[async_trait]` impls match trait macros defined in `systemprompt_traits`; impls cannot deviate.
8. **Typed identifiers** — clean. No raw `String` ID fields; construction via `Id::new`/`generate`; no `.into()`/`::from()` at call sites.
9. **Comment standard** — remediated. Added `//!` heads to 16 `pub mod`/leaf files lacking them; rewrote the archaeology-framed comment in `registry/mod.rs`; split two over-long first doc paragraphs.
10. **No legacy** — clean. No shims, dual paths, or `Option<T>` migration stubs.
11. **Naming** — clean. `*Service`/`*Orchestrator` used; no `*Manager`.
12. **Tests location** — clean. No inline `#[cfg(test)] mod tests`.
13. **Local duplication** — clean. No duplication warranting extraction.
14. **CHANGELOG accuracy** — clean. `CHANGELOG.md` 0.9.2 entry matches the typed-error state in `error.rs`/`shared/error.rs`; doc-only changes here are not user-visible API and need no entry.

### Remediation detail
- `services/shared/resilience.rs`: retry loop discarded the intermediate error (`Err(_)`); now logs `tracing::warn!` before retrying (silent-error anti-pattern fix).
- `//!` heads added: `services/{shared,skills,registry}/mod.rs`, `shared/{auth,config,error,resilience,slug}.rs`, `registry/{security,skills}.rs`, `services/{artifact_publishing,context,execution_tracking,message}.rs`, `services/skills/{skill,skill_injector}.rs`, `models/{agent_info,context,external_integrations,runtime}.rs`, `models/a2a/{mod,jsonrpc}.rs`, `models/a2a/protocol/mod.rs`.

### Verification
- `SQLX_OFFLINE=true cargo clippy -p systemprompt-agent --all-targets --all-features -- -D warnings` — clean.
- `SQLX_OFFLINE=true RUSTDOCFLAGS="-D warnings" cargo doc -p systemprompt-agent --no-deps --all-features` — clean.
