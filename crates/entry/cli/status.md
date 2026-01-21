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
| `src/lib.rs` | 389 | 89 |
| `src/commands/admin/setup/wizard.rs` | 388 | 88 |
| `src/commands/web/validate.rs` | 380 | 80 |
| `src/commands/cloud/dockerfile.rs` | 380 | 80 |
| `src/commands/core/skills/create.rs` | 377 | 77 |
| `src/commands/core/content/types.rs` | 363 | 63 |
| `src/commands/plugins/types.rs` | 352 | 52 |
| `src/commands/plugins/mcp/logs.rs` | 356 | 56 |
| `src/commands/admin/setup/postgres.rs` | 353 | 53 |
| `src/commands/admin/config/types.rs` | 346 | 46 |
| `src/commands/cloud/secrets.rs` | 340 | 40 |
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

### Inline Comments (ZERO TOLERANCE) - FIXED

All inline comments in Rust source files have been removed. Only README.md files contain comments (acceptable for documentation).

### Doc Comments (Require Review)

Some files contain doc comments (`///`, `//!`). Module-level `//!` docs are acceptable when necessary. Function-level `///` docs should be removed.

| File | Count | Status |
|------|-------|--------|
| `src/bootstrap.rs` | 10 | Needs review |
| `src/requirements.rs` | 16 | Acceptable (module docs) |
| `src/routing/remote.rs` | 1 | Acceptable (module doc) |
| `src/commands/plugins/mcp/mod.rs` | 8 | Needs review |
| `src/commands/cloud/secrets.rs` | 3 | Needs review |
| `src/commands/core/content/publish.rs` | 6 | Needs review |
| `src/shared/parsers.rs` | 1 | Acceptable (module doc) |

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

---

## Commands Run

```
cargo clippy -p systemprompt-cli -- -D warnings  # BLOCKED (dependency errors in systemprompt-oauth)
cargo fmt -p systemprompt-cli -- --check          # PASS
```

---

## Summary

| Violation Type | Count | Status |
|----------------|-------|--------|
| Files over 300 lines | 27 | Unfixed |
| Inline comments | 0 | ✅ Fixed |
| Doc comments (needs review) | ~45 | Partially addressed |
| `.ok()` usages | 56 | Unfixed |
| `let _ =` patterns | 7 | Unfixed |
| `unwrap_or_default()` | 3 | Unfixed |
| `Err(_) =>` patterns | 8 | Unfixed |
| Formatting | 0 | ✅ Fixed |

---

## Actions Required

### High Priority (Zero Tolerance)
1. ~~Remove all inline comments~~ ✅ DONE

### Medium Priority
1. Split files over 300 lines into smaller modules (27 files)
2. Review and remove unnecessary doc comments
3. Fix `.ok()` patterns - either propagate errors or log before converting
4. Replace `let _ =` with explicit error handling
5. Replace `unwrap_or_default()` with explicit error handling
6. Replace `Err(_) =>` with proper error propagation or logging

### Blocked
1. Fix systemprompt-oauth clippy errors to unblock CLI clippy checks
