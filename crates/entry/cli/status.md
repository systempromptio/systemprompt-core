# systemprompt-cli Compliance

**Layer:** Entry
**Reviewed:** 2026-01-21
**Verdict:** NON-COMPLIANT

---

## Checklist

| Category | Status |
|----------|--------|
| Boundary Rules | ✅ |
| Required Structure | ✅ |
| Code Quality | ❌ |

---

## Violations

### File Length Violations (Limit: 300 lines)

| File | Lines | Over By |
|------|-------|---------|
| `src/commands/admin/config/rate_limits.rs` | 1425 | 1125 |
| `src/session.rs` | 512 | 212 |
| `src/commands/cloud/deploy/mod.rs` | 400 | 100 |
| `src/lib.rs` | 392 | 92 |
| `src/commands/admin/setup/wizard.rs` | 388 | 88 |
| `src/commands/web/validate.rs` | 380 | 80 |
| `src/commands/cloud/dockerfile.rs` | 380 | 80 |
| `src/commands/core/skills/create.rs` | 377 | 77 |
| `src/commands/core/content/types.rs` | 363 | 63 |
| `src/commands/plugins/types.rs` | 358 | 58 |
| `src/commands/plugins/mcp/logs.rs` | 356 | 56 |
| `src/commands/admin/setup/postgres.rs` | 353 | 53 |
| `src/commands/admin/config/types.rs` | 346 | 46 |
| `src/commands/cloud/secrets.rs` | 341 | 41 |
| `src/commands/cloud/tenant/create.rs` | 334 | 34 |
| `src/commands/admin/setup/docker.rs` | 334 | 34 |
| `src/commands/admin/agents/tools.rs` | 328 | 28 |
| `src/commands/admin/agents/message.rs` | 319 | 19 |
| `src/commands/cloud/tenant/crud.rs` | 315 | 15 |
| `src/commands/plugins/mcp/call.rs` | 314 | 14 |
| `src/commands/infrastructure/system/login.rs` | 308 | 8 |
| `src/commands/infrastructure/services/restart.rs` | 308 | 8 |
| `src/commands/infrastructure/logs/request/stats.rs` | 307 | 7 |
| `src/commands/analytics/overview.rs` | 305 | 5 |
| `src/commands/infrastructure/logs/show.rs` | 304 | 4 |
| `src/commands/admin/agents/logs.rs` | 304 | 4 |
| `src/commands/infrastructure/services/mod.rs` | 302 | 2 |

### Inline Comments (ZERO TOLERANCE)

| File | Count | Category |
|------|-------|----------|
| `src/lib.rs` | 4 | Code Quality |
| `src/commands/content/edit.rs` | 10 | Code Quality |
| `src/commands/plugins/types.rs` | 6 | Code Quality |
| `src/commands/core/content/delete.rs` | 2 | Code Quality |
| `src/commands/core/skills/sync.rs` | 1 | Code Quality |
| `src/commands/core/files/delete.rs` | 2 | Code Quality |
| `src/commands/cloud/secrets.rs` | 1 | Code Quality |
| `src/commands/cloud/domain.rs` | 1 | Code Quality |
| `src/commands/admin/agents/validate.rs` | 1 | Code Quality |
| `src/commands/cloud/deploy/mod.rs` | 1 | Code Quality |
| `src/commands/admin/agents/delete.rs` | 7 | Code Quality |

**Total: 36 inline comments across 11 files**

### Doc Comments (ZERO TOLERANCE except module docs)

| File | Count | Category |
|------|-------|----------|
| `src/bootstrap.rs` | 10 | Code Quality |
| `src/requirements.rs` | 16 | Code Quality |
| `src/routing/remote.rs` | 1 | Code Quality |
| `src/commands/plugins/mcp/list_packages.rs` | 2 | Code Quality |
| `src/commands/plugins/mcp/mod.rs` | 8 | Code Quality |
| `src/commands/plugins/mcp/validate.rs` | 4 | Code Quality |
| `src/commands/admin/agents/delete.rs` | 3 | Code Quality |
| `src/commands/cloud/secrets.rs` | 3 | Code Quality |
| `src/commands/core/content/publish.rs` | 6 | Code Quality |
| `src/shared/parsers.rs` | 1 | Code Quality |
| `src/commands/admin/session/mod.rs` | 1 | Code Quality |
| `src/commands/admin/session/list.rs` | 1 | Code Quality |

**Total: 56 doc comments across 12 files**

### Silent Error Anti-Patterns

| Pattern | File | Line | Category |
|---------|------|------|----------|
| `.ok()` | Multiple files | Various | 56 occurrences across 28 files |
| `let _ =` | `src/commands/infrastructure/services/start.rs` | 76, 83 | Code Quality |
| `let _ =` | `src/commands/infrastructure/services/serve.rs` | 35, 39, 63 | Code Quality |
| `let _ =` | `src/commands/admin/setup/docker.rs` | 114, 115 | Code Quality |
| `unwrap_or_default()` | `src/commands/admin/session/list.rs` | 70 | Code Quality |
| `unwrap_or_default()` | `src/commands/core/content/ingest.rs` | 153, 270 | Code Quality |
| `Err(_) =>` | `src/commands/plugins/mcp/validate.rs` | 182 | Code Quality |
| `Err(_) =>` | `src/commands/admin/config/list.rs` | 22 | Code Quality |
| `Err(_) =>` | `src/commands/admin/setup/postgres.rs` | 225 | Code Quality |
| `Err(_) =>` | `src/commands/cloud/sync/admin_user.rs` | 117 | Code Quality |
| `Err(_) =>` | `src/commands/cloud/status.rs` | 30, 96 | Code Quality |
| `Err(_) =>` | `src/commands/cloud/tenant/mod.rs` | 180 | Code Quality |
| `Err(_) =>` | `src/commands/admin/agents/delete.rs` | 88 | Code Quality |

### Formatting Issues

| Issue | Status |
|-------|--------|
| Import ordering | FAIL - Multiple files need `cargo fmt` |

---

## Commands Run

```
cargo clippy -p systemprompt-cli -- -D warnings  # BLOCKED (dependency errors in systemprompt-oauth)
cargo fmt -p systemprompt-cli -- --check          # FAIL
```

---

## Summary

| Violation Type | Count |
|----------------|-------|
| Files over 300 lines | 27 |
| Inline comments | 36 |
| Doc comments | 56 |
| `.ok()` usages | 56 |
| `let _ =` patterns | 7 |
| `unwrap_or_default()` | 3 |
| `Err(_) =>` patterns | 8 |
| **Total violations** | **193** |

---

## Actions Required

1. Split files over 300 lines into smaller modules
2. Remove all inline comments (code should be self-documenting)
3. Remove doc comments (except rare `//!` module docs where necessary)
4. Fix `.ok()` patterns - either propagate errors or log before converting
5. Replace `let _ =` with explicit error handling
6. Replace `unwrap_or_default()` with explicit error handling or fail fast
7. Replace `Err(_) =>` with proper error propagation or logging
8. Run `cargo fmt -p systemprompt-cli` to fix formatting
9. Fix systemprompt-oauth clippy errors to unblock CLI clippy checks
