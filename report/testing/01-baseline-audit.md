# Test Suite Overview & Health

**Grade: B+** (up from B-)

> **Last updated**: 2026-04-02. Phase 1 COMPLETE.

Well-organized suite across 42 crates with 100% pass rate. Phase 1 cleanup removed ~900 trivial tests, eliminated all weak assertions, split all oversized files, and catalogued error path gaps. Remaining work (coverage measurement, CI, async ratio) is Phase 2+.

---

## Current State

> **Last updated**: 2026-04-01 after Phase 1 cleanup (8 commits on `main`).
> Phase 1 removed ~700 trivial tests, split oversized files, and strengthened weak assertions.

| Metric | Value | Change from Baseline |
|--------|-------|---------------------|
| Total test functions | ~7,209 | -699 from original 7,908 |
| Unit test crates | 28 | — |
| Integration test crates | 14 | — |
| Sync tests (`#[test]`) | ~6,723 (93%) | -698 |
| Async tests (`#[tokio::test]`) | 486 (7%) | -1 |
| Ignored tests | 0 | — |
| `#[should_panic]` tests | 0 | — |
| Compilation status | 42/42 crates compile | — |
| Pass rate | 100% (0 failures) | — |
| Test runner | `cargo test --manifest-path crates/tests/Cargo.toml --workspace` | — |

### Test Distribution by Layer

| Layer | Tests | % of Total | Assessment |
|-------|-------|-----------|------------|
| Domain | 3,459 | 43.7% | Highest volume; mostly model/struct tests, not service logic |
| Infra | 1,532 | 19.4% | Config validation is strong; logging tests are boilerplate |
| Shared | 1,210 | 15.3% | Identifiers alone account for 450 trivial tests |
| App | 849 | 10.7% | Tests data shapes, not orchestration |
| Integration | 549 | 6.9% | Strongest category for behavioral coverage |
| Entry | 309 | 3.9% | API has 83 tests for 152 source files; CLI has 226 for 447 source files |

---

## Desired State

- Maintain 100% green pass rate at all times
- Coverage measurement enabled and reported per PR
- CI pipeline runs the full test suite on every push and pull request
- Coverage badges on README reflecting actual measured coverage
- Async test ratio above 30% to match the async-heavy production surface (services, handlers, middleware)
- Test distribution proportional to code complexity, not code volume

---

## How to Get There

### 1. Unblock Coverage Measurement

The `.cargo/config.toml` configures the Cranelift codegen backend, which conflicts with LLVM-based coverage tools (`cargo llvm-cov`, `cargo tarpaulin`). Two options:

- **Override for CI**: Use `CARGO_PROFILE_DEV_CODEGEN_BACKEND=""` in CI to disable Cranelift during coverage runs
- **Conditional config**: Use a CI-specific `.cargo/config.toml` that omits the Cranelift backend

Once unblocked, add `cargo llvm-cov --workspace --manifest-path crates/tests/Cargo.toml --html` to CI and publish the report as a build artifact.

### 2. Add CI Pipeline

Configure GitHub Actions (or equivalent) with:
- `cargo build --workspace` for compilation check
- `cargo test --manifest-path crates/tests/Cargo.toml --workspace` for test execution
- Coverage reporting with threshold enforcement
- Per-PR coverage diff comments

### 3. Fix the Sync/Async Split

94% sync tests for a codebase where most services, handlers, and middleware are async means the async code paths are largely untested. Priority areas for async tests:
- Database repository methods (all async)
- HTTP handler functions (all async)
- MCP/A2A protocol operations (all async)
- Event broadcasting (async)

### 4. Rebalance Layer Distribution

Entry-layer crates (API and CLI) have the lowest test density despite being the most user-facing. Domain tests are inflated by model boilerplate. Shift effort toward:
- API route handler tests (currently 83 tests for 152 source files)
- CLI command execution tests (currently 226 tests for 447 source files, most are trivial)
- Middleware tests (auth, rate limiting, proxy)

---

## Incremental Improvement Strategy

**Week 1-2**: Unblock coverage measurement. Get a baseline number, even if it is low. Visibility is the prerequisite for all other improvements.

**Week 3-4**: Set up CI pipeline running tests on every push. Add coverage reporting as a non-blocking check. Establish the baseline coverage number in the PR template.

**Month 2**: Convert the 20 highest-value sync tests to async equivalents that exercise real async behavior (database calls, HTTP handlers). Target moving the async ratio from 6% to 15%.

**Month 3**: Set minimum coverage thresholds per layer: 70% for security/auth, 50% for domain services, 30% for entry points. Enforce as CI gates. Ratchet up quarterly.

**Ongoing**: Track coverage trends. Every PR that adds production code should maintain or improve coverage percentage. Every PR that deletes trivial tests should be offset by meaningful tests in undertested areas.
