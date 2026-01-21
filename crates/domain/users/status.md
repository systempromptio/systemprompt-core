# systemprompt-users Compliance

**Layer:** Domain
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
| `src/repository/user/operations.rs` | 322 lines (limit 300) | Code Quality |
| `src/repository/user/list.rs` | 356 lines (limit 300) | Code Quality |
| `src/repository/banned_ip.rs` | 390 lines (limit 300) | Code Quality |
| `src/repository/user/operations.rs:305` | `.unwrap_or_default()` silently swallows error | Silent Error |
| `src/repository/user/list.rs:322` | `.unwrap_or(false)` silently swallows error | Silent Error |
| `src/repository/user/list.rs:144` | `sqlx::query_as(...)` instead of `sqlx::query_as!()` | SQLX Macros |
| `src/repository/user/list.rs:168` | `sqlx::query_as(...)` instead of `sqlx::query_as!()` | SQLX Macros |
| `src/repository/user/list.rs:201` | `sqlx::query_as(...)` instead of `sqlx::query_as!()` | SQLX Macros |
| `src/repository/banned_ip.rs:138` | `sqlx::query(...)` instead of `sqlx::query!()` | SQLX Macros |
| `src/repository/banned_ip.rs:155` | `sqlx::query_as::<_, _>(...)` instead of `sqlx::query_as!()` | SQLX Macros |
| `src/repository/banned_ip.rs:185` | `sqlx::query(...)` instead of `sqlx::query!()` | SQLX Macros |
| `src/repository/banned_ip.rs:221` | `sqlx::query(...)` instead of `sqlx::query!()` | SQLX Macros |
| `src/repository/banned_ip.rs:264` | `sqlx::query(...)` instead of `sqlx::query!()` | SQLX Macros |
| `src/repository/banned_ip.rs:278` | `sqlx::query(...)` instead of `sqlx::query!()` | SQLX Macros |
| `src/repository/banned_ip.rs:293` | `sqlx::query_as::<_, _>(...)` instead of `sqlx::query_as!()` | SQLX Macros |
| `src/repository/banned_ip.rs:321` | `sqlx::query_as::<_, _>(...)` instead of `sqlx::query_as!()` | SQLX Macros |
| `src/repository/banned_ip.rs:351` | `sqlx::query_as::<_, _>(...)` instead of `sqlx::query_as!()` | SQLX Macros |
| `src/repository/banned_ip.rs:378` | `sqlx::query(...)` instead of `sqlx::query_scalar!()` | SQLX Macros |
| `src/jobs/cleanup_anonymous_users.rs:34` | `tracing::info!` without entering `SystemSpan` first | Logging |
| `src/jobs/cleanup_anonymous_users.rs:41` | `tracing::info!` without entering `SystemSpan` first | Logging |

---

## Commands Run

```
cargo clippy -p systemprompt-users -- -D warnings  # PASS
cargo fmt -p systemprompt-users -- --check          # PASS
```

---

## Actions Required

1. **Split `repository/user/operations.rs`** (322 lines) - Extract merge operations to `repository/user/merge.rs`
2. **Split `repository/user/list.rs`** (356 lines) - Extract stats methods to `repository/user/stats.rs`
3. **Split `repository/banned_ip.rs`** (390 lines) - Extract to `repository/banned_ip/mod.rs`, `types.rs`, `queries.rs`
4. **Fix silent error at `operations.rs:305`** - Replace `.unwrap_or_default()` with `?` operator
5. **Fix silent error at `list.rs:322`** - Replace `.unwrap_or(false)` with proper error propagation
6. **Convert SQLX queries in `list.rs`** - Replace `sqlx::query_as(...)` with `sqlx::query_as!()` macro
7. **Convert SQLX queries in `banned_ip.rs`** - Replace all `sqlx::query(...)` and `sqlx::query_as::<_, _>(...)` with macro versions
8. **Add SystemSpan to job** - Wrap job execution in `SystemSpan::new("cleanup_anonymous_users").enter()`

---

## Detailed Findings

### Code Quality

| Metric | Limit | Actual | Status |
|--------|-------|--------|--------|
| Max file length | 300 | 390 | ❌ |
| Cognitive complexity | 15 | <15 | ✅ |
| Function length | 75 | <75 | ✅ |
| Parameters | 5 | 4 | ✅ |

### Forbidden Constructs

| Construct | Status |
|-----------|--------|
| `unsafe` | ✅ None |
| `unwrap()` | ✅ None |
| `panic!()` | ✅ None |
| `todo!()` | ✅ None |
| Inline comments | ✅ None |
| Doc comments | ✅ None |
| TODO/FIXME | ✅ None |

### Silent Error Patterns

| Pattern | Count | Status |
|---------|-------|--------|
| `.ok()` | 0 | ✅ |
| `.unwrap_or_default()` | 1 | ❌ |
| `.unwrap_or(...)` | 1 | ❌ |
| `let _ =` | 0 | ✅ |

### SQLX Compliance

| Query Type | Macro Required | Violations |
|------------|----------------|------------|
| `sqlx::query()` | `sqlx::query!()` | 6 |
| `sqlx::query_as()` | `sqlx::query_as!()` | 3 |
| `sqlx::query_as::<_, _>()` | `sqlx::query_as!()` | 4 |
| `sqlx::query_scalar()` | `sqlx::query_scalar!()` | 1 |

### Architecture

| Rule | Status |
|------|--------|
| No entry layer imports | ✅ |
| Repository pattern | ✅ |
| Services use repositories | ✅ |
| Typed identifiers | ✅ |
| `thiserror` for errors | ✅ |
| `DateTime<Utc>` for timestamps | ✅ |
