# Test File Quality & Standards Compliance

> **Last updated**: 2026-04-01 after Phase 1 cleanup.

## Current State

**Phase 1 is COMPLETE** (2026-04-02). All file size violations resolved, all trivial tests deleted, all weak assertions eliminated.

### File Size Violations — RESOLVED

The project standard for test files is a 500-line limit per file. **Zero violations remain.**

- **0 test files** exceed the 500-line limit (down from 135 over 300 / 20 over 600 / 6 over 1,000).
- 46 files were split across multiple commits into ~160 focused sub-modules.

#### Top 10 Oversized Files (post-cleanup)

| File | Lines | Multiple of Limit |
|------|-------|-------------------|
| `domain/templates/src/registry.rs` | 836 | 2.8x |
| `domain/oauth/src/services/validation/mcp_oauth_flow.rs` | 807 | 2.7x |
| `infra/loader/src/profile_loader.rs` | 768 | 2.6x |
| `domain/analytics/src/models/events.rs` | 748 | 2.5x |
| `infra/config/src/services/validator.rs` | 739 | 2.5x |
| `domain/agent/src/models/a2a.rs` | 701 | 2.3x |
| `domain/analytics/src/models/funnel.rs` | 673 | 2.2x |
| `entry/cli/src/shared/command_result.rs` | 646 | 2.2x |
| `infra/logging/src/models/log_row.rs` | 633 | 2.1x |
| `domain/analytics/src/services/extractor/bot_referrer.rs` | 632 | 2.1x |

### Test Naming Convention

Test naming is consistent and follows the `test_<subject>_<action>` convention throughout. This is a strength -- no remediation needed.

### Brittle Assertion Patterns

Several patterns make tests fragile and likely to break on unrelated changes:

| Pattern | Instances | Risk | Status |
|---------|-----------|------|--------|
| Assertions on exact `Debug` output format | Multiple | Breaks on any `Debug` impl change | 2 debug-only tests deleted in Phase 1 |
| Assertions on exact error message strings | Multiple | Breaks on message rewording | Unchanged |
| `format!("{:?}", x).contains(...)` checks | ~453 | Couples tests to `Debug` formatting | Unchanged (many are legitimate error-testing) |

These patterns test implementation details (how a value is formatted) rather than behavior (what the code does).

### No-Assertion Anti-Pattern

**~1,648 tests construct objects but never assert anything** (down from 1,864 — 216 removed in Phase 1). These tests only verify that code compiles and does not panic at runtime. They do not verify correctness, making them effectively dead weight that inflates test counts without providing confidence.

### Test Module Organization

- Most test files use top-level test functions without `mod tests {}` nesting, which is correct for the separate test crate structure.
- Consistent use of module-per-concern organization.
- Good separation between model tests and service tests within each crate.

## Desired State

- Zero test files exceed the 500-line limit.
- Zero tests use `Debug` format assertions or exact error message string matching.
- Zero tests lack assertions (every test verifies at least one behavioral outcome).
- All tests follow the existing `test_<subject>_<action>` naming convention (already achieved).
- Test files are organized into focused modules with clear separation of concerns.

## How to Get There

### Step 1: Split Oversized Files

~~Start with the 6 files over 1,000 lines.~~ **DONE** — All 6 files over 1,000 lines were split in Phase 1:

- ~~**`app/sync/mod.rs` (2,262 lines)**~~ → split into focused modules (commit `4f028797c`)
- ~~**`app/scheduler/mod.rs` (1,817 lines)**~~ → split into focused modules (commit `4f028797c`)
- ~~**`domain/analytics/src/services/extractor.rs` (1,316 lines)**~~ → split into focused modules (commit `86c72f39f`)
- ~~**`domain/content/src/services/link/mod.rs` (1,254 lines)**~~ → split into focused modules (commit `4f028797c`)
- ~~**`infra/logging/src/trace/models.rs` (1,165 lines)**~~ → split into focused modules (commit `4f028797c`)
- ~~**`domain/users/src/models.rs` (1,158 lines)**~~ → split into focused modules (commit `4f028797c`)
- ~~**`domain/analytics/src/models/core.rs` (1,032 lines)**~~ → split into 3 modules (commit `3f9e02a4d`)
- ~~**`domain/files/src/services/upload.rs` (1,019 lines)**~~ → split into focused modules (commit `4f028797c`)

**Next**: Address the 12 files between 600-1,000 lines, and then the remaining 114 files between 300-600 lines.

### Step 2: Replace Brittle Assertions

For each brittle pattern:

- **Debug format assertions:** Replace with field-level assertions (`assert_eq!(error.code(), expected_code)`).
- **Error message string matching:** Replace with error variant matching (`assert!(matches!(result, Err(MyError::NotFound)))`).
- **`format!("{:?}", x).contains()`:** Replace with structured assertions on the value's fields or with `matches!()`.

### Step 3: Fix No-Assertion Tests

For each of the 1,864 tests with no assertions:

1. Determine what the test should verify (return value, state change, error type).
2. Add at least one assertion that verifies a behavioral outcome.
3. If the test cannot meaningfully assert anything, delete it.

### Step 4: Enforce Standards in CI

Add automated checks:
- File line count check (fail on files exceeding 300 lines).
- Lint for `format!("{:?}"` in test assertions.
- Lint for tests without any `assert` macro invocation.

## Incremental Improvement Strategy

**Week 1:** Split the 6 files over 1,000 lines. This is mechanical work with high impact on maintainability. Target: zero files over 1,000 lines.

**Week 2:** Replace the 31 instances of `format!("{:?}", x).contains(...)` with structured assertions. Audit for additional `Debug` format assertions and fix them. Target: zero brittle format assertions.

**Week 3-4:** Fix no-assertion tests in priority order:
1. Integration tests first (123 tests) -- these are more valuable and fewer in number.
2. Unit tests in domain crates (security, AI, content) -- these cover critical business logic.
3. Unit tests in infrastructure crates.
4. Remaining unit tests in shared crates.

**Week 5:** Split files in the 600-1,000 line range (14 files). Target: zero files over 600 lines.

**Week 6:** Add CI enforcement for file size limits and assertion requirements. This prevents regression.

**Ongoing:** Address the remaining 115 files between 300-600 lines opportunistically -- when modifying a test file for any reason, split it if it exceeds 300 lines.
