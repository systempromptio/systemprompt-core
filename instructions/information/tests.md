# Testing Guidelines

## Test Location

All tests MUST be placed in the separate test workspace located at `crates/tests/`. Tests should NOT be co-located with the source code.

### Why a Separate Test Workspace?

The test crates are in a **separate workspace** (`crates/tests/Cargo.toml`) that is excluded from the main workspace. This provides two key benefits:

1. **Faster downstream builds**: When `systemprompt-core` is used as a git dependency, tests are NOT compiled, significantly reducing build times.
2. **Clean dependency graph**: Consumers of the library don't need to download test-only dependencies.

```
# Main workspace excludes tests
[workspace]
members = ["crates/shared/*", "crates/infra/*", ...]
exclude = ["crates/tests"]

# Test workspace is separate
crates/tests/Cargo.toml  # Own workspace with all test crates
```

### What is NOT Allowed

The following patterns are **strictly prohibited** in source crates:

1. **Inline test modules** - Do NOT use `#[cfg(test)]` blocks in source files:
   ```rust
   // ❌ WRONG - Do NOT do this in source files
   #[cfg(test)]
   mod tests {
       #[test]
       fn test_something() { ... }
   }
   ```

2. **Separate tests directories** - Do NOT create `tests/` folders inside source crates:
   ```
   ❌ WRONG:
   crates/app/generator/tests/my_tests.rs
   crates/entry/cli/tests/integration/test.rs
   ```

3. **Any `#[test]` attribute** - Do NOT place any test functions in source crates

### What IS Allowed

All tests MUST go in `crates/tests/`:

```
✅ CORRECT:
crates/tests/unit/app/generator/my_tests.rs
crates/tests/integration/cli/integration_tests.rs
```

## Unit Test Structure

Unit tests MUST mirror the folder and file structure of the code being tested. The test files should follow the same directory hierarchy as the source files they are testing.

For example:
- Source: `crates/app/runtime/src/context.rs`
- Test: `crates/tests/unit/app/runtime/context.rs`

- Source: `crates/shared/models/src/agent.rs`
- Test: `crates/tests/unit/shared/models/agent.rs`

## Integration Test Structure

Integration tests go in `crates/tests/integration/` organized by feature or domain:

```
crates/tests/integration/
├── a2a/              # Agent-to-agent protocol tests
├── agents/           # Agent lifecycle tests
├── ai/               # AI service tests
├── analytics/        # Analytics and session tests
├── auth/             # Authentication/authorization tests
├── common/           # Shared test utilities
├── content/          # Content management tests
├── database/         # Database constraint tests
├── extension/        # Extension system tests
├── files/            # File handling tests
├── mcp/              # MCP server tests
├── models/           # Model integration tests
├── oauth/            # OAuth flow tests
├── scheduler/        # Job scheduler tests
├── users/            # User management tests
└── traits/           # Trait implementation tests
```

### The 7-Phase Test Pattern

Every integration test MUST follow this structure:

```rust
#[tokio::test]
async fn test_something() -> Result<()> {
    // Phase 1: Setup
    let ctx = TestContext::new().await?;
    let unique_id = ctx.fingerprint().to_string();

    // Phase 2: Action (HTTP request, API call, etc.)
    let response = ctx.make_request("/endpoint").await?;
    assert!(response.status().is_success());

    // Phase 3: Wait for async processing
    wait_for_async_processing().await;

    // Phase 4: Query database to verify state
    let rows = ctx.db.fetch_all(
        &"SELECT * FROM table WHERE id = $1",
        &[&unique_id],
    ).await?;

    // Phase 5: Assert data was persisted correctly
    assert!(!rows.is_empty(), "Data not persisted");
    let data = parse_row(&rows[0])?;
    assert_eq!(data.field, expected_value);

    // Phase 6: Cleanup test data
    let mut cleanup = TestCleanup::new(ctx.db.clone());
    cleanup.track_fingerprint(unique_id);
    cleanup.cleanup_all().await?;

    // Phase 7: Log success
    println!("✓ Test passed with database validation");
    Ok(())
}
```

### Common Test Utilities

Located in `crates/tests/integration/common/`:

| Module | Purpose |
|--------|---------|
| `context.rs` | `TestContext` - centralized test environment setup |
| `assertions.rs` | Fluent assertion builders for domain objects |
| `factories.rs` | Test data builders with realistic defaults |
| `http.rs` | Session extraction, SSE parsing utilities |
| `database.rs` | Async wait, cleanup, validation helpers |
| `cleanup.rs` | `TestCleanup` for removing test data |

### Using TestContext

```rust
use crate::common::*;

#[tokio::test]
async fn test_example() -> Result<()> {
    // Creates test environment with database connection
    let ctx = TestContext::new().await?;

    // Generate unique fingerprint for test isolation
    let fingerprint = ctx.fingerprint().to_string();

    // Make HTTP requests
    let response = ctx.make_request("/api/v1/endpoint").await?;

    // Access database directly
    let rows = ctx.db.fetch_all(&query, &params).await?;

    Ok(())
}
```

### Using TestCleanup

Always clean up test data to prevent pollution:

```rust
let mut cleanup = TestCleanup::new(ctx.db.clone());
cleanup.track_fingerprint(fingerprint);
cleanup.track_user_id(user_id);
cleanup.track_session_id(session_id);
cleanup.cleanup_all().await?;
```

### Key Principles

1. **Every test MUST assert database state**
   ```rust
   // ❌ WRONG: Only checks HTTP status
   assert!(response.status().is_success());

   // ✅ CORRECT: Verifies data persisted
   let rows = ctx.db.fetch_all(&query, &[&id]).await?;
   assert!(!rows.is_empty());
   assert_eq!(rows[0].get::<&str, _>("field"), expected);
   ```

2. **No mega-tests** - One behavior per test
   ```rust
   // ❌ WRONG
   fn test_full_user_lifecycle() { /* 10 assertions */ }

   // ✅ CORRECT
   fn test_user_creation() { }
   fn test_user_update() { }
   fn test_user_deletion() { }
   ```

3. **Cleanup is mandatory** - Never leave test data

4. **Wait for async operations**
   ```rust
   ctx.make_request("/").await?;
   wait_for_async_processing().await;  // Required!
   let rows = ctx.db.fetch_all(...).await?;
   ```

### Running Integration Tests

```bash
# Run all tests in test workspace
cargo test --manifest-path crates/tests/Cargo.toml --workspace

# Single test crate
cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-extension-tests

# With debug logging
RUST_LOG=debug cargo test --manifest-path crates/tests/Cargo.toml --workspace -- --nocapture
```

## Shared Fixtures and Setup

Tests SHOULD share common fixtures and setup code where possible. This promotes consistency and reduces duplication across the test suite. Common utilities should be placed in:

- `crates/tests/integration/common/` - For integration test utilities
- `crates/tests/shared/` - For utilities shared across unit and integration tests

## Linting

Clippy warnings SHOULD be ignored in the testing crate. This allows for more flexible test code without strict linting requirements.

## Code Coverage

Use `cargo-llvm-cov` for fast, accurate code coverage reports.

### Installation

```bash
cargo install cargo-llvm-cov
```

### Running Coverage

```bash
# Quick summary for all tests
cargo llvm-cov --manifest-path crates/tests/Cargo.toml --workspace

# HTML report for detailed analysis
cargo llvm-cov --manifest-path crates/tests/Cargo.toml --workspace --html
# Opens: crates/tests/target/llvm-cov/html/index.html

# JSON output for tooling
cargo llvm-cov --manifest-path crates/tests/Cargo.toml --workspace --json --output-path coverage.json

# Coverage for specific test package
cargo llvm-cov --manifest-path crates/tests/Cargo.toml -p systemprompt-runtime-tests
```

### Coverage Thresholds

Coverage reports should be used to identify untested code paths, not as a strict metric. Focus on testing critical paths and edge cases rather than achieving arbitrary coverage percentages.

## Running Tests Quick Reference

| Task | Command |
|------|---------|
| All tests | `cargo test --manifest-path crates/tests/Cargo.toml --workspace` |
| Single crate | `cargo test --manifest-path crates/tests/Cargo.toml -p <crate>-tests` |
| With output | Add `-- --nocapture` |
| Coverage | `cargo llvm-cov --manifest-path crates/tests/Cargo.toml --workspace` |
| Main workspace only | `cargo test --workspace` (doc tests only) |
