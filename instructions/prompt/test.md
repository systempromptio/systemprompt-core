# Test Coverage Analysis and Implementation

> Analyze test coverage in this crate, identify gaps, fix broken tests, and implement missing test coverage.

---

## Required Reading

Before beginning, you MUST read and understand:

1. **Architecture:** `/var/www/html/systemprompt-core/instructions/information/architecture.md`
2. **Testing Guidelines:** `/var/www/html/systemprompt-core/instructions/information/tests.md`
3. **Rust Standards:** `/var/www/html/systemprompt-core/instructions/prompt/rust.md`

---

## Critical Rules

### Test Location Policy

**ALL tests MUST be in separate crates** - NEVER inline, ALWAYS in the separate test workspace.

| Pattern | Status |
|---------|--------|
| `#[cfg(test)] mod tests { }` in source | FORBIDDEN |
| `crates/domain/users/tests/` | FORBIDDEN |
| `crates/tests/unit/domain/users/` | REQUIRED |

### Test Workspace Structure

```
crates/tests/
├── Cargo.toml              # Separate workspace manifest
├── unit/                   # Unit tests mirroring source structure
│   ├── shared/
│   │   ├── models/
│   │   ├── traits/
│   │   ├── identifiers/
│   │   └── client/
│   ├── infra/
│   │   ├── database/
│   │   ├── logging/
│   │   ├── config/
│   │   ├── cloud/
│   │   ├── loader/
│   │   └── events/
│   ├── domain/
│   │   ├── users/
│   │   ├── oauth/
│   │   ├── files/
│   │   ├── analytics/
│   │   ├── content/
│   │   ├── ai/
│   │   ├── mcp/
│   │   └── agent/
│   ├── app/
│   │   ├── runtime/
│   │   └── generator/
│   └── entry/
│       └── api/
└── integration/            # Integration tests by feature
    └── extension/
```

---

## Analysis Steps

### Phase 1: Inventory Source Crates

For each layer, list all source crates and their key modules:

```bash
# List all source crates
ls -la crates/shared/
ls -la crates/infra/
ls -la crates/domain/
ls -la crates/app/
ls -la crates/entry/
```

### Phase 2: Inventory Test Crates

Map existing test coverage:

```bash
# List all test crates
ls -la crates/tests/unit/
ls -la crates/tests/integration/
```

### Phase 3: Coverage Gap Analysis

For each source crate, determine:

1. **Does a corresponding test crate exist?**
   - Source: `crates/shared/extension/`
   - Expected: `crates/tests/unit/shared/extension/` OR `crates/tests/integration/extension/`

2. **What modules lack test coverage?**
   - Compare `src/` files against test files
   - Identify untested public functions

3. **What is the current test health?**
   ```bash
   cargo test --manifest-path crates/tests/Cargo.toml --workspace 2>&1
   ```

### Phase 4: Coverage Metrics

Run coverage analysis:

```bash
# Install if needed
cargo install cargo-llvm-cov

# Generate coverage report
cargo llvm-cov --manifest-path crates/tests/Cargo.toml --workspace --html

# Quick summary
cargo llvm-cov --manifest-path crates/tests/Cargo.toml --workspace
```

---

## Gap Categories

### Missing Test Crates

Source crates without ANY test coverage:

| Source Crate | Test Crate Status | Priority |
|--------------|-------------------|----------|
| `shared/extension` | MISSING | HIGH |
| `infra/events` | PARTIAL | MEDIUM |
| `app/scheduler` | MISSING | HIGH |
| `app/sync` | MISSING | MEDIUM |

### Missing Test Files

Test crates exist but modules untested:

| Source File | Test Status |
|-------------|-------------|
| `domain/agent/src/services/a2a_server/processing/` | MISSING |
| `domain/ai/src/services/providers/` | PARTIAL |

### Broken Tests

Tests that fail to compile or execute:

| Test File | Error | Fix Required |
|-----------|-------|--------------|
| `tests/unit/domain/agent/...` | Missing import | Add dependency |

---

## Implementation Steps

### Step 1: Fix Broken Tests

For each failing test:

1. Identify the error type (compile error, runtime panic, assertion failure)
2. Trace the root cause
3. Apply minimal fix
4. Verify test passes

### Step 2: Create Missing Test Crates

For each missing test crate:

1. Create `Cargo.toml`:
   ```toml
   [package]
   name = "systemprompt-{name}-tests"
   version.workspace = true
   edition.workspace = true
   publish = false

   [dependencies]
   systemprompt-{name} = { path = "../../../../{layer}/{name}" }

   [dev-dependencies]
   tokio = { workspace = true, features = ["test-util", "macros"] }
   ```

2. Create `src/lib.rs` with module declarations

3. Add to workspace members in `crates/tests/Cargo.toml`

### Step 3: Implement Missing Tests

For each untested module:

1. Mirror the source file structure
2. Test public API only (no implementation details)
3. Cover:
   - Happy path
   - Error cases
   - Edge cases
   - Boundary conditions

---

## Test Implementation Guidelines

### Unit Test Template

```rust
use systemprompt_{crate}::{Module, function_to_test};

#[test]
fn function_name_with_valid_input_returns_expected() {
    let input = create_valid_input();
    let result = function_to_test(input);
    assert_eq!(result, expected_output);
}

#[test]
fn function_name_with_invalid_input_returns_error() {
    let input = create_invalid_input();
    let result = function_to_test(input);
    assert!(result.is_err());
}
```

### Async Test Template

```rust
use systemprompt_{crate}::{AsyncService};

#[tokio::test]
async fn async_operation_completes_successfully() {
    let service = AsyncService::new();
    let result = service.operation().await;
    assert!(result.is_ok());
}
```

### Integration Test Template (7-Phase)

```rust
#[tokio::test]
async fn integration_test_name() -> Result<()> {
    // Phase 1: Setup
    let ctx = TestContext::new().await?;
    let unique_id = ctx.fingerprint().to_string();

    // Phase 2: Action
    let response = ctx.make_request("/endpoint").await?;
    assert!(response.status().is_success());

    // Phase 3: Wait
    wait_for_async_processing().await;

    // Phase 4: Query
    let rows = ctx.db.fetch_all(&query, &[&unique_id]).await?;

    // Phase 5: Assert
    assert!(!rows.is_empty());

    // Phase 6: Cleanup
    let mut cleanup = TestCleanup::new(ctx.db.clone());
    cleanup.track_fingerprint(unique_id);
    cleanup.cleanup_all().await?;

    // Phase 7: Log
    println!("✓ Test passed");
    Ok(())
}
```

---

## Update Crate Status

**MANDATORY:** After completing test work, update `{crate_path}/status.md` with current test state.

### Status File Format

```markdown
# {Crate Name} Status

## Test Status

| Metric | Value |
|--------|-------|
| Test crate | `crates/tests/unit/{layer}/{name}/` |
| Tests passing | X/Y |
| Coverage | Z% |
| Last verified | YYYY-MM-DD |

## Test Commands

\`\`\`bash
# Run tests for this crate
cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-{name}-tests

# Coverage
cargo llvm-cov --manifest-path crates/tests/Cargo.toml -p systemprompt-{name}-tests
\`\`\`

## Known Gaps

- [ ] `module_a.rs` - Missing edge case tests
- [ ] `module_b.rs` - No async tests
```

---

## Output Report

Generate a report with:

### Summary

| Metric | Value |
|--------|-------|
| Total source crates | X |
| Test crates existing | Y |
| Test crates missing | Z |
| Tests passing | A |
| Tests failing | B |
| Line coverage | C% |

### Actions Taken

1. **Fixed:** List of fixed broken tests
2. **Created:** List of new test crates
3. **Implemented:** List of new test files

### Remaining Gaps

| Gap | Priority | Effort |
|-----|----------|--------|
| `shared/extension` full coverage | HIGH | 2 days |
| `domain/agent` A2A protocol tests | HIGH | 3 days |

### Commands to Verify

```bash
# Run all tests
cargo test --manifest-path crates/tests/Cargo.toml --workspace

# Check coverage
cargo llvm-cov --manifest-path crates/tests/Cargo.toml --workspace

# Run specific failing tests
cargo test --manifest-path crates/tests/Cargo.toml -p {crate}-tests -- --nocapture
```

---

## Checklist

Before completing:

- [ ] All tests compile (`cargo build --manifest-path crates/tests/Cargo.toml --workspace`)
- [ ] All tests pass (`cargo test --manifest-path crates/tests/Cargo.toml --workspace`)
- [ ] No inline tests in source crates (`grep -r "#\[cfg(test)\]" crates/shared crates/infra crates/domain crates/app crates/entry`)
- [ ] Test crate naming follows convention (`systemprompt-{name}-tests`)
- [ ] Test files mirror source structure
- [ ] High-priority gaps addressed
- [ ] **Crate `status.md` updated with current test state**
