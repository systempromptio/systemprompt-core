# systemprompt-api Tech Debt Audit

**Layer:** entry
**Audited:** 2026-05-04
**Verdict:** CRITICAL

---

## Summary

| Category | Count |
|----------|-------|
| unwrap()/expect() | 0 |
| panic!()/todo!()/unimplemented!() | 0 |
| println!/eprintln!/dbg! | 0 |
| `let _ =` discards | 5 |
| `.ok()` discards | 62 |
| Inline `//` comments | 0 |
| Doc `///` comments | 0 |
| Files >300 lines | 10 |
| Raw String IDs | 0 |
| Raw `sqlx::query` (outside allowlist) | 5 |
| `*Manager` suffix | 0 |
| `#[allow(...)]` | 27 |
| `anyhow::` references | 99 |
| `async_trait` references | 18 |

**Total scored violations:** 109

---

## Architectural Compliance

Layer: `entry`. Per `instructions/information/boundaries.md` dependencies must flow downward only. This audit does not flag legitimate downward orchestration dependencies.

---

## Passing Checks

| Check | Status |
|-------|--------|
| No `unwrap()` / `expect()` | PASS |
| No `panic!()` / `todo!()` / `unimplemented!()` | PASS |
| No `println!` / `eprintln!` / `dbg!` | PASS |
| No `let _ =` patterns | FAIL (5) |
| No inline `//` comments | PASS |
| No `///` doc comments | PASS |
| All files <=300 lines | FAIL (10) |
| No raw String IDs | PASS |
| No raw `sqlx::query` outside allowlist | FAIL (5) |
| No `*Manager` suffix | PASS |
| No `#[allow(...)]` attributes | FAIL (27) |

---

## File Statistics

| Metric | Value |
|--------|-------|
| Total .rs files | 179 |
| Files over 300 lines | 10 |
| Largest file | `   405 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/jwt/context.rs` |

### Files over 300 lines

```
   307 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/oauth/endpoints/token/generation.rs
   332 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/oauth/endpoints/authorize/validation.rs
   366 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/proxy/backend.rs
   303 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/proxy/engine.rs
   338 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/session.rs
   405 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/jwt/context.rs
   334 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/upstream.rs
   304 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/audit.rs
   379 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/server/routes.rs
   306 /var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/static_content/static_files.rs
```

---

## Offending Locations

### let _ = (fire-and-forget)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/gateway/mod.rs:51:        let _ = repo
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/service.rs:105:                let _ = audit.fail(&msg).await;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/service.rs:127:                let _ = audit.fail(&e.to_string()).await;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/stream_tap.rs:106:                        let _ = audit.fail(&err).await;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/stream_tap.rs:161:                let _ = audit.fail(&msg).await;
```

### .ok() (silent error discard — verify each has logging)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/static_content/static_files.rs:34:        .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/static_content/homepage.rs:21:                    .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/site_auth.rs:59:        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/site_auth.rs:71:                .ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/site_auth.rs:76:                .ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/flatten.rs:87:        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/audit.rs:80:            .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/bot_detector.rs:43:                .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/bot_detector.rs:88:        .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/bot_detector.rs:94:                .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/bot_detector.rs:100:                .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/upstream.rs:87:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/context/extractors/header_extractor.rs:35:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/context/sources/headers.rs:26:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/server/health.rs:33:                .and_then(|v| v.parse::<u64>().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/server/health.rs:39:    let content = std::fs::read_to_string("/proc/self/status").ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/server/health.rs:70:    let stat = nix::sys::statvfs::statvfs(".").ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/server/health.rs:154:    let logs = audit.ok().flatten().map(|row| {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/ip_ban.rs:13:        .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/ip_ban.rs:20:                .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/ip_ban.rs:27:                .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/stream_tap.rs:124:        let snapshot = self.state.lock().ok().and_then(|mut s| {
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/session.rs:100:                let token_result = TokenExtractor::browser_only().extract(headers).ok();
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/session.rs:112:                            .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/negotiation/mod.rs:69:                    .and_then(|q_str| q_str.parse::<f32>().ok().map(|q| q.clamp(0.0, 1.0)))
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/negotiation/mod.rs:105:        .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/jwt/context.rs:177:            self.token_extractor.extract(headers).ok(),
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/jwt/context.rs:239:            .and_then(|h| h.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/jwt/context.rs:247:            .and_then(|h| h.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/jwt/context.rs:255:            .and_then(|h| h.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/auth.rs:102:    let token = TokenExtractor::browser_only().extract(headers).ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/auth.rs:108:    let jwt_secret = SecretsBootstrap::jwt_secret().ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/auth.rs:109:    let config = systemprompt_models::Config::get().ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/auth.rs:123:    let user_id = Uuid::parse_str(&claims.sub).ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/rate_limit.rs:152:                        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/analytics/detection.rs:103:            .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/proxy/engine.rs:91:                .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/proxy/engine.rs:162:                .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/proxy/engine.rs:167:                .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/proxy/engine.rs:195:                        .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/proxy/engine.rs:210:                .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/proxy/engine.rs:234:                    .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/context/extractors/a2a_extractor.rs:50:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/analytics/mod.rs:70:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/middleware/analytics/mod.rs:76:            .and_then(|v| v.to_str().ok())
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/parse.rs:13:    let value = serde_json::from_slice::<Value>(bytes).ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/analytics/events.rs:55:        .ok()
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/gateway/auth.rs:138:    let auth = hdrs.get(headers::AUTHORIZATION)?.to_str().ok()?;
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/sync/auth.rs:20:        .and_then(|v| v.to_str().ok());
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/gateway/messages/rejection.rs:100:    let body_json = serde_json::from_slice::<serde_json::Value>(body).ok();
```

### Raw sqlx::query (outside allowlist)

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/agent/contexts/notifications/handlers.rs:20:    let result = sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/agent/contexts/notifications/handlers.rs:66:                sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/agent/contexts/notifications/handlers.rs:80:                sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/agent/contexts/notifications/handlers.rs:155:    sqlx::query!(
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/agent/contexts/webhook/event_loader.rs:134:    let message = sqlx::query!(
```

### #[allow(...)] attributes

```
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/server/discovery.rs:11:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/server/discovery.rs:76:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/server/discovery.rs:109:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/server/discovery.rs:133:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/server/routes.rs:204:    #[allow(clippy::option_if_let_else)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/audit.rs:36:#[allow(missing_debug_implementations)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/server/health.rs:68:#[allow(clippy::useless_conversion)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/services/gateway/upstream.rs:24:#[allow(missing_debug_implementations)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/proxy/mcp.rs:142:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/sync/files.rs:213:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/sync/files.rs:226:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/sync/files.rs:254:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/admin/keys.rs:131:#[allow(clippy::result_large_err)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/mcp/registry.rs:22:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/oauth/discovery.rs:31:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/oauth/discovery.rs:92:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/oauth/endpoints/webauthn_complete.rs:37:#[allow(unused_qualifications)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/oauth/endpoints/userinfo.rs:32:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/admin/cli.rs:30:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/gateway/messages/extract.rs:12:#[allow(clippy::struct_field_names)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/oauth/webauthn/link/start.rs:28:#[allow(unused_qualifications)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/oauth/webauthn/link/page.rs:4:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/gateway/messages/auth.rs:8:#[allow(clippy::struct_field_names)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/oauth/webauthn/register/start.rs:50:#[allow(unused_qualifications)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/oauth/endpoints/consent.rs:63:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/oauth/health.rs:5:#[allow(clippy::unused_async)]
/var/www/html/systemprompt-core/.claude/worktrees/agent-ac138808aa9458061/crates/entry/api/src/routes/oauth/webauthn/authenticate.rs:34:#[allow(unused_qualifications)]
```

---

## Recommendations for Wave 1/2

- **(W1)** Replace 5 `let _ =` patterns with explicit error logging via `if let Err(e) = ...`.
- **(W2)** Audit 62 `.ok()` calls and ensure each precedes with a `tracing::warn!`/`error!` log of the dropped error.
- **(W1)** Split 10 files exceeding 300 lines into focused submodules.
- **(W1)** Convert 5 raw `sqlx::query` calls to compile-time-verified `sqlx::query!`/`query_as!`/`query_scalar!` macros (or move into the `admin/`/`postgres ext` allowlist if dynamic SQL is intentional).
- **(W2)** Remove 27 `#[allow(...)]` attributes by fixing the underlying clippy/rustc warnings.

---

## Verdict

**CRITICAL**

Other Wave 1 agents are concurrently fixing source code; final CLEAN status will be re-validated after the wave merges.
