# Testing Instructions

Achieve at least 90% code coverage for the specified crate by writing comprehensive unit tests.

## Prerequisites

Before starting, read and understand the testing guidelines:
- `/var/www/html/systemprompt-core/instructions/information/tests.md`

## Step 1: Review Testing Rules

1. **Test Location**: All tests go in `crates/tests/unit/{layer}/{crate_name}/`
   - Example: Tests for `crates/app/runtime/` go in `crates/tests/unit/app/runtime/`

2. **Forbidden Patterns**:
   - NO `#[cfg(test)]` blocks in source files
   - NO `tests/` directories inside source crates
   - NO `#[test]` functions in source crates

3. **Test Structure**: Mirror the source file structure
   - Source: `crates/{layer}/{name}/src/foo.rs`
   - Test: `crates/tests/unit/{layer}/{name}/src/foo.rs`

## Step 2: Get Current Coverage

Run coverage for the target crate:

```bash
cargo llvm-cov --manifest-path crates/tests/Cargo.toml -p {package_name}-tests
```

Example output to analyze:
```
Filename                      Lines    Cover
src/context.rs                172      9.30%
src/validation.rs             27       0.00%
```

Record the current coverage percentage for each file.

## Step 3: Identify Coverage Gaps

For each file with <90% coverage:

1. Read the source file to understand the code
2. Identify untested:
   - Public functions and methods
   - Error handling paths
   - Edge cases and boundary conditions
   - Match arms and conditionals

## Step 4: Create Test Crate (if needed)

If `crates/tests/unit/{layer}/{crate_name}/` doesn't exist:

1. Create `Cargo.toml`:
```toml
[package]
name = "systemprompt-{crate_name}-tests"
version.workspace = true
edition.workspace = true
publish = false

[dependencies]
systemprompt-{crate_name} = { path = "../../../../{layer}/{crate_name}" }
# Add other test dependencies as needed

[lints]
workspace = true
```

2. Create `src/lib.rs` with module declarations

3. Add the test crate to the workspace `Cargo.toml`

## Step 5: Write Tests

For each untested function/path:

1. **Name tests descriptively**: `test_{function}_{scenario}_{expected}`
2. **Test happy paths first**: Normal successful operations
3. **Test error cases**: Invalid inputs, edge cases
4. **Test boundary conditions**: Empty inputs, max values, etc.

Example test structure:
```rust
#[test]
fn test_validate_config_returns_error_for_empty_name() {
    let config = Config { name: "".to_string(), ..Default::default() };
    let result = validate_config(&config);
    assert!(result.is_err());
}

#[test]
fn test_validate_config_succeeds_for_valid_input() {
    let config = Config { name: "valid".to_string(), ..Default::default() };
    let result = validate_config(&config);
    assert!(result.is_ok());
}
```

## Step 6: Verify Coverage

After writing tests, re-run coverage:

```bash
cargo llvm-cov --manifest-path crates/tests/Cargo.toml -p {package_name}-tests
```

Target: **≥90% line coverage** for each file.

If below 90%:
1. Identify remaining uncovered lines
2. Write additional tests
3. Repeat until target is reached

## Step 7: Run All Tests

Ensure all tests pass:

```bash
cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-{crate_name}-tests
```

Fix any failures before completing.

## Completion Criteria

The testing task is complete when:

1. ✅ Test crate exists at correct location
2. ✅ Tests follow naming conventions
3. ✅ No tests in source crates
4. ✅ All tests pass
5. ✅ Coverage ≥90% for each source file
6. ✅ Edge cases and error paths covered

## Quick Reference

| Command | Purpose |
|---------|---------|
| `cargo llvm-cov --manifest-path crates/tests/Cargo.toml -p {pkg}-tests` | Coverage for single crate |
| `cargo llvm-cov --manifest-path crates/tests/Cargo.toml -p {pkg}-tests --html` | HTML coverage report |
| `cargo test --manifest-path crates/tests/Cargo.toml -p {pkg}-tests` | Run tests for crate |
| `cargo test --manifest-path crates/tests/Cargo.toml -p {pkg}-tests -- --nocapture` | Run with output |
