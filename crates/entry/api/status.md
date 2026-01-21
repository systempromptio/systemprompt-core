# Status Report: systemprompt-api

**Layer:** Entry
**Review Date:** 2026-01-21
**Verdict:** NON-COMPLIANT (39 remaining violations)

---

## Summary

The `systemprompt-api` crate serves as the HTTP API gateway for SystemPrompt OS. Initial review found 62 violations. After fixes, 39 violations remain.

---

## Fixed This Session

| Category | Fixed | Method |
|----------|-------|--------|
| `.expect()` violations | 13 | Replaced with `http::HeaderValue::from_static()`, `unwrap_or(NonZeroU32::MIN)`, or `?` propagation |
| Doc comments | 3 | Removed |
| Inline comments | 2 | Removed |
| Boundary violations | 3 | Refactored routes to return `Result<Router>` with proper error propagation |
| Formatting issues | 8 | Ran `cargo fmt` |

**Total Fixed:** 29

---

## Remaining Violations

### 1. `unwrap_or`/`unwrap_or_else` Silent Fallbacks (26 violations)

Per rust.md: "unwrap_or() hiding failures - Return `Err` or log explicitly before fallback"

| File | Line | Issue |
|------|------|-------|
| `services/server/builder.rs` | 258 | Silent fallback to hardcoded path |
| `services/server/lifecycle/scheduler.rs` | 23 | Silent fallback with logging only |
| `services/server/lifecycle/scheduler.rs` | 28 | Silent fallback with logging only |
| `services/server/lifecycle/scheduler.rs` | 121 | Fallback to "Unknown error" |
| `services/server/readiness.rs` | 65 | Silent fallback to false |
| `services/middleware/rate_limit.rs` | 78-79 | u32 overflow fallback |
| `services/middleware/rate_limit.rs` | 150 | Silent IP fallback |
| `services/middleware/session.rs` | 198 | Extension extraction fallback |
| `services/middleware/analytics/detection.rs` | 21,32,40,48 | Silent error swallowing |
| `services/middleware/analytics/detection.rs` | 61,109 | Fallback defaults |
| `services/middleware/context/middleware.rs` | 165 | Auth level fallback |
| `services/middleware/context/sources/payload.rs` | 20 | Empty method fallback |
| `services/middleware/bot_detector.rs` | 29 | Empty string fallback |
| `services/middleware/throttle.rs` | 36 | Silent error with logging |
| `routes/analytics/events.rs` | 29-30 | URL parsing fallback |
| `routes/admin/cli.rs` | 99,163 | PID/exit code fallback |
| `routes/engagement/handlers.rs` | 37-38 | URL parsing fallback |
| `services/proxy/client.rs` | 18 | Fallback to default client |
| `services/proxy/auth.rs` | 29 | Boolean fallback |

### 2. `let _ =` Pattern (13 violations)

Per rust.md: "let _ = result - Handle error explicitly or use `?`"

| File | Line | Context |
|------|------|---------|
| `services/server/runner.rs` | 20,30 | Event send results discarded |
| `routes/admin/cli.rs` | 113,128,151 | Stream/kill results discarded |
| `services/server/builder.rs` | 48 | Event send discarded |
| `services/server/routes.rs` | 168,182,192,293 | Warning event sends discarded |
| `services/middleware/analytics/mod.rs` | 159 | Database update discarded |
| `services/server/readiness.rs` | 34,41 | Broadcast sends discarded |

---

## Code Quality Metrics

### File Size (Lines > 300)

| File | Lines | Status |
|------|-------|--------|
| `services/server/builder.rs` | 392 | EXCEEDS 300 |
| `services/static_content/vite.rs` | 333 | EXCEEDS 300 |
| `services/server/routes.rs` | 312 | EXCEEDS 300 |
| `services/proxy/backend.rs` | 302 | EXCEEDS 300 |

---

## Checklist Summary

| Check | Status |
|-------|--------|
| No `.unwrap()` | PASS |
| No `.expect()` | PASS (fixed) |
| No `panic!()` | PASS |
| No silent fallbacks | FAIL (26) |
| No discarded results | FAIL (13) |
| No inline comments | PASS (fixed) |
| No doc comments | PASS (fixed) |
| No TODO/FIXME | PASS |
| No direct SQL | PASS |
| Routes use services only | PASS (fixed) |
| Files < 300 lines | FAIL (4) |
| `cargo fmt` clean | PASS (fixed) |
| `cargo clippy` clean | BLOCKED (dependency errors) |

---

## Commands Run

```
cargo fmt -p systemprompt-api -- --check   # PASS
cargo clippy -p systemprompt-api           # BLOCKED (oauth crate compilation errors)
```

---

## Required Actions for Compliance

### High Priority

1. **Silent fallback patterns (26):** Add explicit logging before fallback or propagate errors
2. **Discarded results (13):** Handle channel send failures or document why ignoring is safe
3. **Large files (4):** Consider splitting files exceeding 300 lines

### Resolution Approach

For `let _ =` with channel sends:
```rust
// Current (violation):
let _ = tx.send(event);

// Fixed (log on failure):
if tx.send(event).is_err() {
    tracing::debug!("Event receiver dropped");
}
```

For `unwrap_or` patterns:
```rust
// Current (violation):
.unwrap_or("")

// Fixed (log before fallback):
.map_err(|e| {
    tracing::warn!(error = %e, "Parse failed");
    e
}).unwrap_or("")
```

---

**Verdict: NON-COMPLIANT**

39 violations remain. See resolution approach above.
