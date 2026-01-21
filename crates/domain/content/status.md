# systemprompt-content Compliance

**Layer:** Domain
**Reviewed:** 2026-01-21
**Verdict:** NON-COMPLIANT

---

## Checklist

| Category | Status |
|----------|--------|
| Boundary Rules | ✅ |
| Required Structure | ❌ |
| Repository Quality | ✅ |
| Service Quality | ✅ |
| Idiomatic Rust | ✅ |
| Code Quality | ❌ |

---

## Violations

| File:Line | Violation | Category |
|-----------|-----------|----------|
| `src/repository/content/mod.rs` | 329 lines (limit: 300) | Code Quality |
| `src/services/ingestion/mod.rs` | 324 lines (limit: 300) | Code Quality |
| `src/config/validated.rs:231` | `unwrap_or_default()` usage | Code Quality |
| `src/analytics/repository.rs` | Duplicate of `repository/link/analytics.rs` | Code Quality |
| `src/analytics/service.rs` | Duplicate of `services/link/analytics.rs` | Code Quality |
| `module.yaml` | Missing required file | Required Structure |

---

## Commands Run

```
cargo clippy -p systemprompt-content -- -D warnings  # PASS
cargo fmt -p systemprompt-content -- --check          # PASS
```

---

## Actions Required

1. Split `src/repository/content/mod.rs` to reduce below 300 lines
2. Split `src/services/ingestion/mod.rs` to reduce below 300 lines
3. Replace `unwrap_or_default()` at `src/config/validated.rs:231` with explicit handling
4. Remove duplicate `src/analytics/` module - consolidate with `src/repository/link/` and `src/services/link/`
5. Create `module.yaml` at crate root with required fields (name, version, display_name, type)

---

## Compliance Summary

### Boundary Rules ✅

- ✅ No cross-domain imports
- ✅ No app layer imports
- ✅ No entry layer imports
- ✅ Only shared/ and infra/ dependencies

### Required Structure ❌

- ❌ `module.yaml` missing
- ✅ `src/repository/` exists
- ✅ `src/services/` exists
- ✅ `src/error.rs` exists
- ✅ README.md exists
- ✅ status.md exists

### Repository Quality ✅

- ✅ All queries use SQLX macros (`query!`, `query_as!`)
- ✅ No runtime query strings
- ✅ No business logic in repositories
- ✅ Typed IDs used (ContentId, LinkId, etc.)
- ✅ Pool is `Arc<PgPool>`

### Service Quality ✅

- ✅ Repositories injected via constructor
- ✅ Errors mapped to domain error types (ContentError)
- ✅ No direct SQL in services
- ✅ Uses tracing for logging

### Idiomatic Rust ✅

- ✅ Iterator chains over imperative loops
- ✅ `?` operator for error propagation
- ✅ `thiserror` for domain error types
- ✅ Builder pattern used for complex types

### Code Quality ❌

- ❌ File length violations (2 files >300 lines)
- ❌ `unwrap_or_default()` anti-pattern (1 occurrence)
- ❌ Duplicate code (analytics module duplicates link analytics)
- ✅ No `unsafe`
- ✅ No `unwrap()` / `panic!()`
- ✅ No inline comments
- ✅ No TODO/FIXME
- ✅ cargo clippy passes
- ✅ cargo fmt passes
