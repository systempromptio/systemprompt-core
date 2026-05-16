# systemprompt-api Audit — Area 2: gateway + agent routes

Scope: `crates/entry/api/src/routes/gateway/` and `crates/entry/api/src/routes/agent/`.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Routes depend only downward (runtime, domain, infra, models); no sideways/circular deps. |
| 2 | Error model | clean | `anyhow` used in entry crate as permitted; handlers return `Result<_, (StatusCode, String)>` or `Response`. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!` anywhere in scope. |
| 4 | Raw SQL | clean | No SQL in handlers; all DB access via repositories (`ContextRepository`, `ServiceRepository`, etc.). |
| 5 | File size | remediated | `bridge_manifest.rs` (460 lines) split into `bridge_manifest/{mod,skills,agents,hooks}.rs`, all under limit. |
| 6 | Function size | clean | Three handlers slightly over 75 lines (`registry` 92, `context_broadcast` 99, `notifications/mod` 120) are cohesive flat error-match handlers; splitting would only add helper padding. Left as-is. |
| 7 | Async traits | clean | No `#[async_trait]` in scope; native `async fn` throughout. |
| 8 | Typed identifiers | clean | Typed IDs used (`AgentId`, `SkillId`, `HookId`, `UserId`, `JwtToken`). `&str` passed to repos whose signatures are `&str` (cross-crate, out of scope). No `.into()`/`::from()` at call sites. |
| 9 | Comment standard | clean | No per-item `///` in scope. New `//!` module heads added to split files; one substantive WHY-comment on `CanonicalView` preserved. |
| 10 | No legacy | clean | No shims, dual paths, stubs, or dead code. |
| 11 | Naming | clean | No `*Manager`. Handlers named `handle`/`manifest`/`router` etc. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | No notable copy-paste within scope. |
| 14 | CHANGELOG | clean | Not edited (observation only). |

## Remediation summary

Only item 5 required change. `bridge_manifest.rs` exceeded the 300-line file limit; it was
split via `git mv` into a `bridge_manifest/` module:

- `mod.rs` — `manifest` handler, `CanonicalView`, auth/version helpers.
- `skills.rs` — `load_skills` skill discovery.
- `agents.rs` — `load_agents` agent projection.
- `hooks.rs` — `load_hooks` hook discovery.

No behavioural change. `pub mod bridge_manifest;` in `gateway/mod.rs` resolves to the new
directory module unchanged.

Verified: `cargo clippy -p systemprompt-api --all-targets --all-features -D warnings` clean;
`cargo doc -p systemprompt-api --no-deps` clean.
