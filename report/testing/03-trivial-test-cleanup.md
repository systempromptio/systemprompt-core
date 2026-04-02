# Trivial Test Analysis

**Grade: B** (up from D)

> **Last updated**: 2026-04-02. Phase 1 COMPLETE.

Phase 1 removed all Send/Sync tests (105), all Debug-only tests (33), all PartialEq derived trait tests (434), 291 excess serde round-trip tests, and 29 no-assertion tests — totalling 892 trivial tests deleted. Remaining serde round-trip tests (~243) are retained because they test types with custom `#[serde(...)]` attributes.

---

## Current State

### Trivial Test Breakdown

| Category | Count | % of Suite | Description | Phase 1 Status |
|----------|-------|-----------|-------------|----------------|
| No assertions at all | ~1,648 | 23% | Functions that create structs or call methods without checking results | 216 removed |
| Serde serialize/deserialize | 534 | 7% | Round-trip tests on types that derive `Serialize`/`Deserialize` | Unchanged |
| Equality/PartialEq tests | 495 | 7% | Tests verifying `x == x.clone()` on derived `PartialEq` types | Unchanged |
| Send/Sync trait tests | 0 | 0% | Tests asserting a type is `Send` or `Sync` -- compiler guarantees | **All 105 deleted** |
| Debug format-only tests | 0 | 0% | Tests that format a value with `{:?}` and check it contains a string | **All deleted** (was 31, then 2 more in Phase 1 Wave 1) |
| **Total trivial** | **~2,313** | **32%** | | -716 removed |

### No-Assertion Tests by Layer

The 1,864 tests with zero assertions are distributed as follows:

| Layer / Crate | No-Assertion Tests | Total Tests in Crate | % No-Assertion |
|---------------|-------------------|---------------------|----------------|
| unit/domain | 1,024 | 3,459 | 30% |
| unit/infra | 275 | 1,532 | 18% |
| unit/app | 210 | 849 | 25% |
| integration/users | 84 | 84 | **100%** |
| unit/shared | 80 | 1,210 | 7% |
| unit/entry | 58 | 309 | 19% |
| integration/extension | 53 | 222 | 24% |
| integration/files | 39 | 39 | **100%** |
| integration/content | 22 | - | - |
| integration/analytics | 8 | - | - |
| integration/agents | 5 | - | - |
| integration/scheduler | 3 | - | - |
| integration/database | 2 | - | - |
| integration/models | 1 | - | - |

### Critical Problem Areas

- **integration/files**: All 39 tests have zero assertions. The entire file integration test suite verifies nothing.
- **integration/users**: All 84 tests have zero assertions. User management integration tests are completely hollow.
- **unit/domain**: 1,024 no-assertion tests make this the single largest source of noise in the suite.

---

## Desired State

- Every test function contains at least one meaningful assertion that verifies behavior, not structure
- Tests for `Clone`, `Debug`, `Eq`, `Default`, `Send`, `Sync` on types that derive these traits do not exist
- Serde round-trip tests use property-based testing (`proptest`) with varied inputs, not hardcoded examples
- Zero tests with no assertions
- The test count may drop from 7,908 to ~5,000, but effective coverage increases

---

## How to Get There

### ~~Phase 1: Delete Send/Sync Tests (105 tests)~~ DONE

~~These are the easiest to justify removing.~~ All 105 Send/Sync tests deleted in commit `9677f1ac3`. One compile-time helper was preserved (valid use).

### ~~Phase 2: Delete Derived Trait Tests (~500 tests)~~ PARTIALLY DONE

~~Tests that verify `Clone`, `Debug`, `Eq`, `Default` on types using `#[derive(...)]` are testing the Rust compiler.~~ All 31+ Debug format-only tests deleted (commits `9677f1ac3`, `3fc159e6e`). The ~495 PartialEq tests and serde round-trip tests remain as future work.

### Phase 3: Fix or Delete No-Assertion Tests (~1,648 tests)

This is the largest and most important phase. For each no-assertion test:

1. **If the test exercises meaningful behavior** (calls a service method, builds a complex object, processes input): add assertions that verify the output, side effects, or state changes. These tests were likely written as scaffolding and never completed.

2. **If the test only constructs a struct or calls a trivial constructor**: delete it. Struct construction is verified by the compiler.

Priority order:
- integration/users (84 tests) -- these should be real integration tests with database assertions
- integration/files (39 tests) -- these should verify file operations produce correct results
- unit/domain (1,024 tests) -- triage individually; many will be deletions, some will need assertions added

### Phase 4: Convert Serde Tests to Property-Based (534 tests)

Replace hardcoded serde round-trip tests with `proptest` generators:

```rust
proptest! {
    #[test]
    fn roundtrip_task_status(status: TaskStatus) {
        let json = serde_json::to_string(&status).unwrap();
        let decoded: TaskStatus = serde_json::from_str(&json).unwrap();
        assert_eq!(status, decoded);
    }
}
```

One property test replaces dozens of hardcoded examples and catches edge cases that manual tests miss.

---

## Incremental Improvement Strategy

~~**Week 1**: Delete all 105 Send/Sync tests.~~ **DONE** (commit `9677f1ac3`).

~~**Week 2**: Delete derived-trait tests.~~ **PARTIALLY DONE** — Debug-only tests deleted (commits `9677f1ac3`, `3fc159e6e`). PartialEq tests (~495) remain.

**Next**: Triage the 84 integration/users and 39 integration/files no-assertion tests. These are integration tests that should be the highest-value tests in the suite. Either add real database/filesystem assertions or delete them and file issues to write proper replacements.

**Then**: Work through the ~1,024 unit/domain no-assertion tests. Batch by crate: analytics models, content models, agent models. For each batch, categorize as "needs assertions" or "delete" and execute.

**Later**: Add `proptest` dependency to the test workspace. Convert serde round-trip tests one crate at a time, starting with the crates that have the most serialization surface (agent, ai, mcp).

**Tracking metric**: Monitor the ratio of (tests with assertions) / (total tests). Current ratio: ~68% (up from ~62%). Target: 100%.
