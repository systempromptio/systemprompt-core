# Clippy Lint Exceptions

**Policy:** Inline `#[allow(clippy::...)]` is forbidden except for cases listed here.

Each exception requires:
1. Technical constraint that cannot be resolved through refactoring
2. Clear documentation of why the lint cannot be satisfied
3. Commitment to revisit if Rust/clippy evolves

---

## Approved Exceptions

### 1. Serde Empty Struct Serialization

**Lint:** `empty_structs_with_brackets`

**Location:** `crates/domain/ai/src/models/providers/gemini/request.rs`

**Structs:** `GoogleSearch`, `UrlContext`, `CodeExecution`

**Justification:** Gemini API requires these fields serialized as `{}` (empty JSON object), not `null`. Serde requires `struct Foo {}` syntax to produce `{}`. Using `struct Foo;` produces `null`.

```rust
#[allow(clippy::empty_structs_with_brackets)]
pub struct GoogleSearch {}
```

**Status:** APPROVED - External API constraint

---

### 2. CLI Fatal Exit

**Lint:** `exit`

**Location:** `crates/infra/logging/src/services/cli/mod.rs:77`

**Function:** `CliOutput::fatal()`

**Justification:** CLI entry point must terminate process on unrecoverable errors. This is the intended behavior for command-line tools. The function is marked `-> !` (never returns).

```rust
#[allow(clippy::exit)]
pub fn fatal(message: &str, exit_code: i32) -> !
```

**Status:** APPROVED - CLI entry point design

---

### 3. Capability Flags Struct

**Lint:** `struct_excessive_bools`

**Location:** `crates/domain/ai/src/services/schema/capabilities.rs:4`

**Struct:** `ProviderCapabilities`

**Justification:** This struct represents JSON Schema feature flags for different AI providers. Each boolean maps directly to a specific schema capability (allOf, anyOf, oneOf, etc.). Using an enum or bitflags would obscure the domain model and complicate provider-specific initialization.

```rust
#[allow(clippy::struct_excessive_bools)]
pub struct ProviderCapabilities {
    pub supports_allof: bool,
    pub supports_anyof: bool,
    // ... 7 more capability flags
}
```

**Status:** APPROVED - Domain model clarity

---

### 4. RwLock Entry Pattern

**Lint:** `significant_drop_tightening`

**Location:** `crates/infra/events/src/services/broadcaster.rs:70`

**Function:** `GenericBroadcaster::register()`

**Justification:** The lock must be held across `entry().or_default().insert()` chain. Clippy suggests dropping the lock earlier, but this would create a race condition where the entry could be removed between operations.

```rust
#[allow(clippy::significant_drop_tightening)]
async fn register(&self, user_id: &UserId, connection_id: &str, sender: EventSender) {
    let mut connections = self.connections.write().await;
    connections.entry(user_id.to_string()).or_default().insert(...);
}
```

**Status:** APPROVED - Concurrency correctness

---

### 5. Debug Impl with Unprintable Field

**Lint:** `missing_fields_in_debug`

**Location:** `crates/infra/events/src/services/broadcaster.rs:153`

**Struct:** `ConnectionGuard<E>`

**Justification:** The `broadcaster` field is `&'static Lazy<GenericBroadcaster<E>>` which cannot be meaningfully displayed. Using `finish_non_exhaustive()` is the correct pattern for this case.

```rust
#[allow(clippy::missing_fields_in_debug)]
impl<E: ToSse + Clone + Send + Sync + 'static> std::fmt::Debug for ConnectionGuard<E>
```

**Status:** APPROVED - Static reference cannot be displayed

---

### 6. Static Regex Initialization

**Lint:** `expect_used`

**Location:** `crates/shared/models/src/profile/mod.rs:39`

**Static:** `ENV_VAR_REGEX`

**Justification:** Static `Lazy<Regex>` initializers cannot propagate errors with `?`. The regex pattern is a compile-time constant that will always succeed. Using `.ok()` (the previous pattern) silently converts to `Option<Regex>`, making all env var substitution silently fail if the regex somehow failed. `expect()` with a descriptive message is the correct pattern for infallible static initialization.

```rust
#[allow(clippy::expect_used)]
static ENV_VAR_REGEX: Lazy<Regex> = Lazy::new(|| {
    Regex::new(r"\$\{(\w+)\}")
        .expect("ENV_VAR_REGEX is a valid regex - this is a compile-time constant")
});
```

**Status:** APPROVED - Static initializer constraint

---

## Technical Debt (Pending Refactoring)

The following allows are currently in the codebase but require refactoring to remove:

### cognitive_complexity (13 occurrences) - DENIED

These functions exceed complexity limits and must be refactored:

| Location | Function |
|----------|----------|
| `crates/app/scheduler/src/services/scheduling/mod.rs:36` | `start` |
| `crates/app/scheduler/src/services/scheduling/mod.rs:79` | `register_single_job` |
| `crates/app/scheduler/src/services/scheduling/mod.rs:174` | `handle_job_result` |
| `crates/app/scheduler/src/jobs/behavioral_analysis.rs:154` | `log_flag_result` |
| `crates/app/scheduler/src/jobs/behavioral_analysis.rs:195` | `log_ban_result` |
| `crates/domain/content/src/repository/content/mod.rs:22` | `create` |
| `crates/domain/content/src/repository/link/mod.rs:26` | `create_link` |
| `crates/domain/content/src/repository/link/analytics.rs:173` | `record_click` |
| `crates/domain/content/src/jobs/content_ingestion.rs:134` | `log_validation_error` |
| `crates/domain/content/src/api/routes/query.rs:34` | `execute_search` |
| `crates/domain/content/src/analytics/repository.rs:173` | `record_click` |
| `crates/domain/analytics/src/repository/session/mutations.rs:179` | `create_session` |
| `crates/domain/analytics/src/repository/ml_features.rs:20` | `insert_features` |
| `crates/domain/analytics/src/repository/engagement.rs:20` | `create_engagement` |
| `crates/infra/logging/src/repository/analytics/mod.rs:46` | `run_insert_query` |

**Refactoring strategy:** Extract match arms into helper functions. Use early returns. Break into smaller single-purpose functions.

---

## Summary

| Category | Approved | Pending Refactor |
|----------|----------|------------------|
| Serde constraints | 3 | 0 |
| CLI entry points | 1 | 0 |
| Domain modeling | 1 | 0 |
| Concurrency correctness | 1 | 0 |
| Debug implementation | 1 | 0 |
| Static initializer | 1 | 0 |
| cognitive_complexity | 0 | 13 |
| **Total** | **8** | **13** |

---

## Removed Allows (This Session)

The following inline allows were removed as redundant or fixed:

| Lint | Location | Resolution |
|------|----------|------------|
| `cast_lossless` | `tui/draw/panels.rs:72` | Allowed at workspace level |
| `cast_possible_truncation` | `tui/draw/panels.rs:102` | Allowed at workspace level |
| `cast_precision_loss` | `tui/draw/overlays.rs:66` | Allowed at workspace level |
| `cast_lossless` | `tui/draw/overlays.rs:72` | Allowed at workspace level |
| `cast_*` (multiple) | `tui/draw/overlays.rs:74-79` | Allowed at workspace level |
| `use_self` | `models/artifacts/metadata.rs:97` | Fixed: use `Self` |
| `use_self` | `models/artifacts/metadata.rs:147` | Fixed: use `Self` |
| `case_sensitive_file_extension_comparisons` | `security/scanner.rs` | Fixed: use `Path::extension()` with `eq_ignore_ascii_case()` |
| `collection_is_never_read` | `analytics/service.rs:75` | False positive |
| `result_large_err` | `agent/api/routes/contexts/mod.rs:24` | Nursery lint (warn) |
| `type_complexity` | `logging/trace/service.rs:50` | Warn level |
| `literal_string_with_formatting_args` | `logging/cli/mod.rs:263` | Different templating syntax |
| `missing_const_for_fn` | `content/models/builders/content.rs:100` | Cannot be const |
| `missing_const_for_fn` | `analytics/session_cleanup.rs:12` | Cannot be const |
| `missing_const_for_fn` | `logging/maintenance.rs:14` | Cannot be const |
