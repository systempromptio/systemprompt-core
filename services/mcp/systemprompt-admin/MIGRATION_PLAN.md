# systemprompt-mcp-admin Migration Plan

Migration to latest systemprompt-core API.

## Summary

58 compilation errors across the following categories:

| Category | Count | Complexity |
|----------|-------|------------|
| `with_data` returns Result | 25 | Medium |
| Import path changes | 6 | Low |
| API signature changes | 5 | Medium |
| Field renames | 2 | Low |
| Other | 20 | Varies |

## 1. Import Path Changes (6 errors)

### 1.1 `systemprompt_models` → `systemprompt::models`
**File:** `src/tools/conversations/sections.rs:95`
```rust
// Old
systemprompt_models::artifacts::types::SortOrder::Desc
// New
systemprompt::models::artifacts::types::SortOrder::Desc
```

### 1.2 `systemprompt::system::models` moved
**File:** `src/tools/dashboard/sections.rs:2`
```rust
// Old
use systemprompt::system::models::analytics::PlatformOverview;
// New - find new location in core
```

### 1.3 `systemprompt::system::repository` moved
**File:** `src/tools/dashboard/mod.rs:8`
```rust
// Old
use systemprompt::system::repository::analytics::CoreStatsRepository;
// New - find new location in core
```

### 1.4 `TaskRepository` moved
**File:** `src/tools/conversations/mod.rs:7`
```rust
// Old
use systemprompt::agent::repository::TaskRepository;
// New - check agent module exports
```

### 1.5 `scheduler::services::jobs` moved
**File:** `src/tools/jobs/mod.rs:7`
```rust
// Old
use systemprompt::scheduler::services::jobs;
// New - check scheduler module exports
```

### 1.6 `ConfigLoader` removed
**File:** `src/tools/operations/validation.rs:5`
```rust
// Old
use systemprompt::models::ConfigLoader;
// New - use serde_yaml::from_str directly or find replacement
```

### 1.7 `static_content` module moved
**File:** `src/tools/jobs/mod.rs:123`
```rust
// Old
systemprompt::scheduler::services::static_content::optimize_images(pool).await
// New - find new location
```

## 2. `DashboardSection::with_data` Returns Result (25 errors)

The `with_data` method now returns `Result<Self, serde_json::Error>` instead of `Self`.

### Pattern to fix:
```rust
// Old
DashboardSection::new("id", "Title", SectionType::Table)
    .with_data(data)
    .with_layout(layout)

// New
DashboardSection::new("id", "Title", SectionType::Table)
    .with_data(data)?
    .with_layout(layout)
```

### Files affected:
- `src/tools/traffic/sections.rs` (6 occurrences)
- `src/tools/dashboard/sections.rs` (6 occurrences)
- `src/tools/content/sections.rs` (4 occurrences)
- `src/tools/conversations/sections.rs` (3 occurrences)
- `src/tools/logs/sections.rs` (2 occurrences)
- `src/tools/operations/mod.rs` (1 occurrence)
- `src/tools/operations/validation.rs` (3 occurrences)

### Function signature changes required:
Functions calling `with_data` must return `Result` to use `?`:
```rust
// Old
pub fn create_section() -> DashboardSection

// New
pub fn create_section() -> Result<DashboardSection, serde_json::Error>
```

## 3. Field Renames (2 errors)

### 3.1 `mcp_server_name` → `server_name`
**File:** `src/server/handlers/tools.rs`
```rust
// Old
ToolExecutionRequest {
    mcp_server_name: self.service_id.to_string(),
    ...
}
// New
ToolExecutionRequest {
    server_name: self.service_id.to_string(),
    ...
}
```

### 3.2 `A2aArtifact.artifact_id` field changed
**File:** Check usage and update field name

## 4. API Signature Changes (5 errors)

### 4.1 `ToolUsageRepository::new` takes reference
```rust
// Old
ToolUsageRepository::new(DbPool::clone(&self.db_pool))
// New
ToolUsageRepository::new(&self.db_pool)?
```

### 4.2 `AuthenticatedRequestContext` access pattern
**File:** Check `.context` field access - may need `?` or different access pattern

### 4.3 `JobsService::list_enabled_jobs` signature changed
**File:** `src/tools/jobs/mod.rs`
Check new signature and update call

### 4.4 Function argument count changes
- One method takes 1 argument but 10 were supplied
- One function takes 2 arguments but 3 were supplied
Check specific call sites

## 5. Result Propagation (13 errors)

Many functions now need to return `Result` to propagate errors from:
- `with_data()?`
- `ToolUsageRepository::new()?`
- Other fallible operations

### Pattern:
```rust
// Old
pub fn my_function() -> MyType {
    section.with_data(data)
}

// New
pub fn my_function() -> Result<MyType, Error> {
    Ok(section.with_data(data)?)
}
```

## 6. Execution Order

1. **Fix imports first** - These are quick wins
2. **Fix field renames** - Simple find/replace
3. **Fix API signatures** - Update function calls
4. **Fix with_data calls** - Add `?` operator
5. **Update function signatures** - Make functions return Result
6. **Update callers** - Propagate Result handling up the call stack

## 7. Testing

After fixes:
```bash
cargo build --release
cargo clippy -- -D warnings
cargo test
```

## 8. Files to Modify

| File | Changes |
|------|---------|
| `src/server/handlers/tools.rs` | ToolUsageRepository, server_name, auth context |
| `src/tools/dashboard/mod.rs` | Import paths |
| `src/tools/dashboard/sections.rs` | Import paths, with_data |
| `src/tools/traffic/sections.rs` | with_data |
| `src/tools/content/sections.rs` | with_data |
| `src/tools/conversations/mod.rs` | Import paths |
| `src/tools/conversations/sections.rs` | Import paths, with_data |
| `src/tools/logs/sections.rs` | with_data |
| `src/tools/jobs/mod.rs` | Import paths, API changes |
| `src/tools/operations/mod.rs` | with_data |
| `src/tools/operations/validation.rs` | ConfigLoader, with_data |
