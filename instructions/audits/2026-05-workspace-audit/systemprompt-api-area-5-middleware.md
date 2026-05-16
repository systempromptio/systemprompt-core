# systemprompt-api Audit — Area 5: `src/services/middleware/`

Scope: `crates/entry/api/src/services/middleware/**` (auth, JWT, context, session,
analytics, bot detection, rate limiting, CORS, security headers, throttle, trace,
trailing-slash, content negotiation). Entry binary crate — `anyhow` permitted,
per-item `///` rustdoc banned.

| # | Item | Status | Note |
|---|------|--------|------|
| 1 | Layering | clean | Depends only downward on domain/infra/shared crates; no sideways or circular deps. |
| 2 | Error model | clean | `anyhow::Result` at constructors (entry crate); typed `ApiError`/`ContextExtractionError`/`CorsError` elsewhere. |
| 3 | No panics | clean | No `unwrap`/`expect`/`panic!`/`dbg!`/`println!`/`eprintln!`; fallible conversions use `unwrap_or`/`?`/`ok()`. |
| 4 | Raw SQL | clean | No SQL run here; all DB access goes through repository types (`SessionRepository`, `BannedIpRepository`, etc.). |
| 5 | File size | clean | Largest file `session.rs` 240 lines; all under the 300-line limit. |
| 6 | Function size | clean | All functions within ~75-line guidance; long flows already split into cohesive sub-modules. |
| 7 | Async traits | clean | `#[async_trait]` on `ContextExtractor` — required for `dyn`/`Arc<dyn>` use in `ContextMiddleware`; consistent across impls. |
| 8 | Typed identifiers | clean | Typed IDs throughout; constructed via `Id::new`/`try_new`/`generate`, no `.into()`/`::from()` at call sites. |
| 9 | Comment standard | remediated | Stripped banned per-item `///` on `decode_for_gateway` (jwt/context.rs); added a WHY justification for the `serde_json::Value` protocol boundary in `payload.rs`. |
| 10 | No legacy | remediated | Removed dead snake_case re-export alias `JwtExtractor as jwt_extractor` in `jwt/mod.rs` (unreferenced). |
| 11 | Naming | clean | `*Middleware`/`*Service`/`*Extractor`/`*Source` — no `*Manager`. |
| 12 | Tests location | clean | No inline `#[cfg(test)] mod tests`. |
| 13 | Local duplication | clean | Client-IP extraction repeats between `bot_detector.rs` and `ip_ban.rs` but they are independent middleware modules; left as-is to avoid a cross-module helper (no behavioural change in scope). |
| 14 | CHANGELOG | clean | Not edited (observations only). |

## Remediations applied
- `jwt/context.rs`: removed banned `///` doc block from `decode_for_gateway`.
- `site_auth.rs`: removed redundant `use tracing;` import (all call sites fully qualified).
- `context/sources/payload.rs`: added WHY comment justifying `serde_json::Value` at the A2A JSON-RPC protocol boundary.
- `jwt/mod.rs`: removed dead `JwtExtractor as jwt_extractor` re-export alias.

No behavioural changes; auth/middleware semantics unchanged.
