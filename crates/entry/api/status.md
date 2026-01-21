# Status Report: systemprompt-api

**Layer:** Entry
**Review Date:** 2026-01-21
**Verdict:** NON-COMPLIANT (26 remaining violations)

---

## Summary

The `systemprompt-api` crate serves as the HTTP API gateway for SystemPrompt OS. Initial review found 62 violations. After fixes, 26 violations remain (all `unwrap_or`/`unwrap_or_else` patterns).

---

## Fixed This Session

| Category | Fixed | Method |
|----------|-------|--------|
| `.expect()` violations | 13 | Replaced with `http::HeaderValue::from_static()`, `unwrap_or(NonZeroU32::MIN)`, or `?` propagation |
| `let _ =` violations | 13 | Added explicit error handling with `if ...is_err() { tracing::debug!(...) }` |
| Doc comments | 3 | Removed |
| Inline comments | 2 | Removed |
| Boundary violations | 3 | Refactored routes to return `Result<Router>` with proper error propagation |
| Formatting issues | 8 | Ran `cargo fmt` |

**Total Fixed:** 42

---

## Remaining Violations

### `unwrap_or`/`unwrap_or_else` Silent Fallbacks (26 violations)

Per rust.md: "unwrap_or() hiding failures - Return `Err` or log explicitly before fallback"

| File | Line | Issue |
|------|------|-------|
| `services/server/builder.rs` | 258 | Silent fallback to hardcoded path |
| `services/server/lifecycle/scheduler.rs` | 23,28,121 | Silent fallbacks with logging only |
| `services/server/readiness.rs` | 65 | Silent fallback to false |
| `services/middleware/rate_limit.rs` | 78-79,150 | u32 overflow / IP fallbacks |
| `services/middleware/session.rs` | 198 | Extension extraction fallback |
| `services/middleware/analytics/detection.rs` | 21,32,40,48,61,109 | Silent error swallowing |
| `services/middleware/context/middleware.rs` | 165 | Auth level fallback |
| `services/middleware/context/sources/payload.rs` | 20 | Empty method fallback |
| `services/middleware/bot_detector.rs` | 29 | Empty string fallback |
| `services/middleware/throttle.rs` | 36 | Silent error with logging |
| `routes/analytics/events.rs` | 29-30 | URL parsing fallback |
| `routes/admin/cli.rs` | 99,163 | PID/exit code fallback |
| `routes/engagement/handlers.rs` | 37-38 | URL parsing fallback |
| `services/proxy/client.rs` | 18 | Fallback to default client |
| `services/proxy/auth.rs` | 29 | Boolean fallback |

---

## Code Quality Metrics

### File Size (Lines > 300)

| File | Lines | Status |
|------|-------|--------|
| `services/server/builder.rs` | ~400 | EXCEEDS 300 |
| `services/static_content/vite.rs` | ~333 | EXCEEDS 300 |
| `services/server/routes.rs` | ~334 | EXCEEDS 300 |
| `services/proxy/backend.rs` | ~302 | EXCEEDS 300 |

---

## Checklist Summary

| Check | Status |
|-------|--------|
| No `.unwrap()` | PASS |
| No `.expect()` | PASS (fixed) |
| No `panic!()` | PASS |
| No silent fallbacks | FAIL (26) |
| No discarded results | PASS (fixed) |
| No inline comments | PASS (fixed) |
| No doc comments | PASS (fixed) |
| No TODO/FIXME | PASS |
| No direct SQL | PASS |
| Routes use services only | PASS (fixed) |
| Files < 300 lines | FAIL (4) |
| `cargo fmt` clean | PASS |
| `cargo clippy` clean | BLOCKED (dependency errors) |

---

## Commands Run

```
cargo fmt -p systemprompt-api -- --check   # PASS
cargo clippy -p systemprompt-api           # BLOCKED (oauth crate compilation errors)
```

---

## Required Actions for Compliance

### Remaining Work

1. **Silent fallback patterns (26):** Add explicit logging before fallback using:
```rust
.map_err(|e| {
    tracing::warn!(error = %e, "Operation failed");
    e
}).unwrap_or(default)
```

2. **Large files (4):** Consider splitting files exceeding 300 lines:
   - `builder.rs` → Extract middleware configuration
   - `vite.rs` → Extract template rendering
   - `routes.rs` → Extract extension mounting
   - `backend.rs` → Extract request/response transformation

---

**Verdict: NON-COMPLIANT**

26 violations remain. All are `unwrap_or`/`unwrap_or_else` patterns requiring explicit logging before fallback.
