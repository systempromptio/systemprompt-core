# systemprompt-ai Compliance

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
| `src/services/providers/gemini/tools.rs` | 402 lines (limit: 300) | Code Quality |
| `src/services/providers/gemini_images.rs` | 314 lines (limit: 300) | Code Quality |
| `src/services/core/image_service.rs` | 305 lines (limit: 300) | Code Quality |
| `src/services/core/request_storage/async_operations.rs:12` | `.ok()` silently swallows database errors | Silent Error |
| `src/services/core/request_storage/async_operations.rs:21` | `let _ =` ignores message insert errors | Silent Error |
| `src/services/core/request_storage/async_operations.rs:33` | `let _ =` ignores tool call insert errors | Silent Error |
| `src/services/core/request_storage/async_operations.rs:63` | `let _ =` ignores session usage update errors | Silent Error |
| `src/services/core/request_storage/async_operations.rs:73` | `.unwrap_or(false)` hides session check errors | Silent Error |
| `src/services/core/request_storage/async_operations.rs:81` | `.unwrap_or(3600)` hardcoded fallback without error logging | Silent Error |
| `src/services/core/request_storage/async_operations.rs:109` | `let _ =` ignores session creation errors | Silent Error |
| `src/services/providers/gemini_images.rs:32` | `.unwrap_or_else()` fallback without logging | Silent Error |

---

## Commands Run

```
cargo clippy -p systemprompt-ai -- -D warnings  # BLOCKED (upstream dependency)
cargo fmt -p systemprompt-ai -- --check          # PASS (auto-fixed)
```

Note: Clippy is blocked by unrelated errors in `systemprompt-runtime` dependency. The AI crate itself has no clippy violations visible.

---

## Actions Required

1. **Split `gemini/tools.rs` (402 lines)**: Extract `ToolRequestParams`, `ToolResultParams` and builders into separate `params.rs` module
2. **Split `gemini_images.rs` (314 lines)**: Extract request building and response parsing into helper modules
3. **Split `image_service.rs` (305 lines)**: Extract persistence logic into dedicated module
4. **Fix silent errors in `async_operations.rs`**: Add `tracing::error!` logging before `.ok()` and `let _ =` patterns per rust.md standards:
   ```rust
   // Current (violation)
   repo.insert(record).await.ok()

   // Fixed (compliant)
   repo.insert(record).await.map_err(|e| {
       tracing::error!(error = %e, "Failed to store AI request");
       e
   }).ok()
   ```
5. **Fix `gemini_images.rs:32`**: Log client creation failure before fallback

---

## Boundary Compliance (Verified)

| Rule | Status |
|------|--------|
| No entry layer imports (`systemprompt-api`, `systemprompt-tui`) | ✅ |
| No direct SQL in services | ✅ |
| Services use repositories | ✅ |
| Business logic delegated properly | ✅ |

---

## Idiomatic Rust Compliance (Verified)

| Rule | Status |
|------|--------|
| Iterator chains over imperative loops | ✅ |
| `?` operator for error propagation | ✅ |
| No unnecessary `.clone()` | ✅ |
| `impl Into<T>` / `AsRef<T>` for flexible APIs | ✅ |
| Builder pattern for complex types | ✅ |
| No `unsafe` blocks | ✅ |
| No `unwrap()` / `panic!()` | ✅ |
| No TODO/FIXME comments | ✅ |
