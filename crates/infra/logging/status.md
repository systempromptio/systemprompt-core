# systemprompt-logging Compliance

**Layer:** Infrastructure
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

| File:Line | Violation | Category |
|-----------|-----------|----------|
| `src/lib.rs:28-29` | Doc comments (`///`) on static variable | Code Quality |
| `src/trace/models.rs:55` | Doc comment (`///`) on struct field | Code Quality |
| `src/services/cli/display.rs` | 449 lines (limit: 300) | Code Quality |
| `src/services/cli/mod.rs` | 333 lines (limit: 300) | Code Quality |
| `src/trace/queries.rs` | 329 lines (limit: 300) | Code Quality |
| `src/trace/ai_trace_queries.rs` | 323 lines (limit: 300) | Code Quality |
| `src/services/cli/theme.rs` | 310 lines (limit: 300) | Code Quality |

---

## Commands Run

```
cargo clippy -p systemprompt-logging -- -D warnings  # PASS
cargo fmt -p systemprompt-logging -- --check          # PASS
```

---

## Actions Required

1. Remove doc comment from `src/lib.rs:28-29` (static `LOGGING_INITIALIZED`)
2. Remove doc comment from `src/trace/models.rs:55` (`error_message` field)
3. Split `src/services/cli/display.rs` (449 lines) into smaller modules
4. Split `src/services/cli/mod.rs` (333 lines) into smaller modules
5. Split `src/trace/queries.rs` (329 lines) into smaller modules
6. Split `src/trace/ai_trace_queries.rs` (323 lines) into smaller modules
7. Split `src/services/cli/theme.rs` (310 lines) into smaller modules
