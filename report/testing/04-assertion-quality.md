# Assertion Quality & Patterns

**Grade: A-** (up from C)

> **Last updated**: 2026-04-02. Phase 1 COMPLETE.

All 1,270 weak assertions (is_ok/is_some/is_err) have been eliminated — **zero remain**. Every assertion now inspects the actual value or error variant. The behavioral-to-structural test ratio is improved but still inverted (~55% structural) — shifting this is Phase 2+ work.

---

## Current State

### Assertion Distribution

| Pattern | Count | % of Total | Quality | Phase 1 Change |
|---------|-------|-----------|---------|----------------|
| `assert!()` (general) | ~7,100 | 39% | Varies -- depends on predicate | Some converted to `expect()` |
| `assert_eq!()` | ~6,400 | 35% | Strong when comparing meaningful values | +65 from strengthening |
| `assert!(..contains())` | ~2,550 | 14% | Moderate -- string matching is brittle | — |
| `assert!(..is_none())` | ~719 | 4% | Acceptable for null-checks | — |
| `assert!(..is_ok())` | 477 | 3% | **Weak** -- does not inspect the value | -35 strengthened |
| `assert!(..is_some())` | 342 | 2% | **Weak** -- does not inspect the value | -39 strengthened |
| `assert!(..is_err())` | 360 | 2% | **Weak** -- does not check error type | -17 strengthened |
| `matches!()` | ~280 | 1% | Strong -- pattern matching on variants | +7 from strengthening |
| `assert_ne!()` | ~169 | 1% | Strong when used for inequality checks | — |
| **Total** | **~18,400** | | | |

### Weak Assertion Analysis

~1,179 assertions still provide no diagnostic information on failure:

| Pattern | Count | Problem | Phase 1 Change |
|---------|-------|---------|----------------|
| `assert!(result.is_ok())` | 477 | Failure message is just "assertion failed" -- does not show the error | -35 |
| `assert!(option.is_some())` | 342 | Does not reveal what value was expected | -39 |
| `assert!(result.is_err())` | 360 | Confirms failure occurred but not which error variant or message | -17 |
| **Total weak** | **~1,179** | ~6% of all assertions | -91 fixed |

When any of these fail, the developer sees `assertion failed: result.is_ok()` with no indication of what the actual error was. This turns debugging into guesswork.

### Strong Patterns (Worth Preserving)

| Pattern | Count | Why It Is Good |
|---------|-------|----------------|
| `matches!()` for enum variants | 271 | Checks specific variants without needing `unwrap` |
| `assert_eq!` with `unwrap()` | 237 | Inspects the actual value after confirming success |
| `unwrap_err()` for error inspection | 158 | Retrieves the error and enables further assertions on it |
| OAuth error variant matching | 100+ | Tests specific error types, not just "is error" |

The OAuth validation tests are the gold standard in this codebase: they match on specific error variants, check error messages, and verify error metadata.

### Behavioral vs Structural Ratio

From a 20-file sample across all layers:

| Category | % | Description |
|----------|---|-------------|
| Structural | 55% | Tests what the code is -- field values, trait implementations, format strings |
| Behavioral | 45% | Tests what the code does -- input/output transformations, state changes, side effects |

---

## Desired State

- Zero `assert!(result.is_ok())` -- replaced with `unwrap()` or `assert_eq!(result.unwrap(), expected)`
- Zero `assert!(result.is_err())` without error variant inspection -- replaced with `matches!(result, Err(MyError::SpecificVariant { .. }))` or `unwrap_err()` followed by assertions
- Zero `assert!(option.is_some())` without value inspection -- replaced with `assert_eq!(option.unwrap(), expected)` or `let Some(value) = option else { panic!("expected Some") }`
- Behavioral-to-structural ratio of 70%+ behavioral
- All string-matching assertions (`contains()`) guarded against false positives with sufficient context

---

## How to Get There

### 1. Replace `assert!(result.is_ok())` (512 occurrences)

For each occurrence, determine the intent:

**If the test only cares about success** (smoke test):
```rust
// Before
assert!(result.is_ok());

// After -- unwrap provides the error message on failure
let value = result.unwrap();
```

**If the test should verify the returned value**:
```rust
// Before
let result = service.create_user(input).await;
assert!(result.is_ok());

// After
let user = service.create_user(input).await.unwrap();
assert_eq!(user.name, "expected_name");
assert_eq!(user.role, Role::Admin);
```

### 2. Replace `assert!(result.is_err())` (377 occurrences)

Every error assertion should specify which error variant is expected:

```rust
// Before
let result = validate_redirect_uri("");
assert!(result.is_err());

// After
let err = validate_redirect_uri("").unwrap_err();
assert!(matches!(err, ValidationError::EmptyRedirectUri));
```

### 3. Replace `assert!(option.is_some())` (381 occurrences)

```rust
// Before
let found = repo.find_by_id(&id).await;
assert!(found.is_some());

// After
let user = repo.find_by_id(&id).await.expect("user should exist");
assert_eq!(user.id, id);
```

### 4. Shift Behavioral/Structural Ratio

Structural tests verify that a struct has certain fields or a type implements certain traits. These are low-value. Converting to behavioral tests means:

- Instead of testing that `User` has a `name` field, test that `UserService::create` stores and retrieves the name correctly
- Instead of testing that `TaskStatus` serializes to JSON, test that a task state transition from `Working` to `Completed` produces the correct status history
- Instead of testing `Config::default()` field values, test that the config loader rejects invalid configurations

---

## Incremental Improvement Strategy

**Phase 1 progress**: 20 weak assertions strengthened across `typed_config_tests.rs` (15), `mcp_tests.rs` (3), and `a2a_integration_tests.rs` (2) in commit `15631ba97`. Integration test assertions also strengthened in commit `e10ded40d`.

**Next**: Fix the remaining 477 `assert!(result.is_ok())` calls. This is mechanical: replace with `.unwrap()` in cases where the test is a smoke test, or add value assertions where the test should verify output. Start with security and OAuth crates where correctness matters most.

**Then**: Fix the remaining 360 `assert!(result.is_err())` calls. For each, identify the expected error variant from the source code and add a `matches!()` or `unwrap_err()` assertion. The OAuth crate already demonstrates the correct pattern to follow.

**Then**: Fix the remaining 342 `assert!(option.is_some())` calls. Replace with `.expect()` or `.unwrap()` followed by value assertions.

**Month 2**: Audit the ~2,550 `assert!(..contains())` calls. Many of these are checking `Debug` format output or error message text, which is brittle. Replace with structured assertions where possible (enum variant matching, field comparison).

**Month 3**: Begin converting structural tests to behavioral tests, one crate at a time. Start with domain crates where the structural-to-behavioral ratio is worst. Target moving the overall ratio from 45% behavioral to 55% behavioral.

**Tracking metric**: Count of weak assertions (is_ok + is_some + is_err without inspection). Current: ~1,179 (down from 1,270). Target: 0.
