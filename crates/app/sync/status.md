# systemprompt-sync Compliance

**Layer:** Application
**Reviewed:** 2026-01-21
**Verdict:** NON-COMPLIANT

---

## Checklist

| Category | Status |
|----------|--------|
| Boundary Rules | ❌ |
| Required Structure | ✅ |
| Code Quality | ❌ |
| Orchestration Quality | ❌ |
| Idiomatic Rust | ❌ |

---

## Violations

| File:Line | Violation | Category |
|-----------|-----------|----------|
| `src/lib.rs:1-19` | Excessive `#![allow(...)]` clippy suppressions (18 lints) | Code Quality |
| `src/lib.rs:209` | Direct `std::env::var()` instead of Config pattern | Anti-Pattern |
| `src/database.rs:1` | File exceeds 300 lines (373 lines) | Code Quality |
| `src/database.rs:217` | `unwrap_or(false)` silently defaults | Silent Error |
| `src/database.rs:316` | `unwrap_or(false)` silently defaults | Silent Error |
| `src/database.rs:334` | `unwrap_or(false)` silently defaults | Silent Error |
| `src/diff/content.rs:116` | `.ok()` without logging before conversion | Silent Error |
| `src/diff/content.rs:21` | Direct repository instantiation | Orchestration |
| `src/diff/skills.rs:20` | Direct repository instantiation | Orchestration |
| `src/local/skills_sync.rs:34` | Direct repository instantiation | Orchestration |
| `src/local/skills_sync.rs:115` | Direct repository instantiation | Orchestration |
| `src/local/content_sync.rs:50` | Direct repository instantiation | Orchestration |
| `src/local/content_sync.rs:129` | Direct repository instantiation | Orchestration |
| `src/lib.rs:48` | Derive ordering: should be `Clone, Copy, Debug, ...` | Idiomatic Rust |
| `src/api_client.rs:7` | Derive ordering: should be `Clone, Debug` | Idiomatic Rust |
| `src/files.rs:14,21,28` | Derive ordering: should be `Clone, Debug, ...` | Idiomatic Rust |
| `src/database.rs:9,17,34,49,59` | Derive ordering violations | Idiomatic Rust |
| `src/models/local_sync.rs:4,10,17,29` | Derive ordering violations | Idiomatic Rust |
| `src/export/skills.rs:13` | `unwrap_or("skills")` hardcoded fallback | Anti-Pattern |
| `src/local/skills_sync.rs:121` | "delete not implemented" - incomplete functionality | Code Quality |

---

## Commands Run

```
cargo clippy -p systemprompt-sync -- -D warnings  # PASS
cargo fmt -p systemprompt-sync -- --check          # PASS
```

---

## Actions Required

1. **Remove excessive clippy allows** - Fix the underlying issues instead of suppressing 18 lints in `lib.rs`
2. **Split database.rs** - Extract upsert functions to separate file to stay under 300 lines
3. **Replace `unwrap_or(false)` patterns** - Use proper error propagation with `?` or explicit logging
4. **Fix `.ok()` in diff/content.rs:116** - Log error before converting to Option
5. **Use domain services** - Replace direct `Repository::new()` calls with domain service injection
6. **Fix derive ordering** - Use consistent ordering: `Clone, Copy, Debug, Default, PartialEq, Eq, Hash, Serialize, Deserialize`
7. **Replace `std::env::var()`** - Use Config pattern from `systemprompt-config`
8. **Remove hardcoded fallbacks** - `unwrap_or("skills")` should fail explicitly or use typed constants
9. **Implement delete functionality** - Complete the TODO in skills_sync.rs or remove the feature
