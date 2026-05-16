# systemprompt-api — Area 7 (misc) audit

Scope: `src/services/static_content/`, `src/services/health/`, `src/services/mod.rs`,
`src/services/validation.rs`, `src/models/`, `src/lib.rs`, `CHANGELOG.md`.

1. Layering — clean. Only downward deps (runtime/domain/infra/shared); no sideways or circular imports.
2. Error model — clean. Entry crate; `anyhow::Result` used at handler/service boundaries as permitted.
3. No panics — clean. No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!` in scope.
4. Raw SQL — clean. No SQL executed; DB access goes through `ContentRepository`/`ServiceRepository`.
5. File size — clean. Largest in-scope file is `health/monitor.rs` at 219 lines (<300).
6. Function size — clean. All functions within the ~75-line guidance.
7. Async traits — clean. No trait definitions in scope; all `async fn` are free functions/methods.
8. Typed identifiers — remediated. `ClientId::new("sp_web".to_string())` simplified to `ClientId::new("sp_web")` (redundant `.to_string()`); other `Id::new` calls pass already-owned strings.
9. Comment standard — clean. No `///` in scope (banned in entry binary); no narrative `//` comments.
10. No legacy — clean. No shims, dual paths, stubs, or dead code.
11. Naming — clean. `HealthChecker`, `ProcessMonitor`, `*Service` usages; no `*Manager`.
12. Tests location — clean. No inline `#[cfg(test)] mod tests`.
13. Local duplication — clean. `serve_homepage` overlaps `serve_cached_file` but keeps a distinct 404 body and read-error message; extracting would change behaviour, so left as-is.
14. CHANGELOG accuracy — clean. Recent entries (0.9.2, 0.3.0, 0.2.2) match code; no stale claims in scope.
