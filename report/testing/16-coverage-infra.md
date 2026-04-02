# Infrastructure Layer Coverage

## Current State

The infrastructure layer has the best test coverage in the codebase. Seven crates span security, events, database, config, cloud, logging, and loader functionality. Coverage ranges from excellent (security at 90%+) to moderate with quality concerns (logging with 67% no-op tests).

### Security (crates/infra/security/) -- EXCELLENT

- **Source**: 13 files
- **Tests**: 13 test files, 259 tests
- **Coverage**: ~90%+ (1:1 file ratio)
- **Quality**: Tests cover extraction logic, validation, and error cases thoroughly.
- **Assessment**: Best-tested crate in the entire project. A model for other crates to follow.

### Events (crates/infra/events/) -- GOOD

- **Source**: ~8 files
- **Tests**: 4 test files, 87 tests (42 sync, 45 async)
- **Coverage**: ~85%
- **Quality**: Broadcaster, routing, SSE, and connection guard are all tested. Async tests properly verify event-driven behavior.

### Database (crates/infra/database/) -- GOOD

- **Source**: ~12 files
- **Tests**: 17 test files, 173 tests
- **Coverage**: ~80%
- **Quality**: Repository patterns, error handling, and value conversion are tested.
- **Gap**: No integration tests for connection pool behavior under load.

### Config (crates/infra/config/) -- GOOD

- **Source**: ~10 files
- **Tests**: 7 test files, 177 tests
- **Coverage**: ~85%
- **Quality**: Validator tests are strong with good error coverage (25 error assertions).
- **Gap**: Schema validation could test more edge cases.

### Cloud (crates/infra/cloud/) -- MODERATE

- **Source**: ~15 files
- **Tests**: 10 test files, 310 tests
- **Coverage**: ~65%
- **Quality**: Model tests are comprehensive.
- **Gap**: Cloud API interaction logic is undertested.

### Logging (crates/infra/logging/) -- MODERATE (Inflated)

- **Source**: ~12 files
- **Tests**: 17 test files, 409 tests
- **Coverage**: Numbers look good but quality is poor.
- **Issue**: 275 tests have NO assertions (67% of logging tests are no-ops). The 260 trace/models.rs tests mostly construct structs without verifying behavior. Only the CLI theme tests have meaningful assertions.
- **Assessment**: Effective coverage is significantly lower than raw numbers suggest.

### Loader (crates/infra/loader/) -- MODERATE

- **Source**: ~8 files
- **Tests**: 8 test files, 117 tests
- **Coverage**: ~70%
- **Quality**: Profile loader and include resolver tested with error paths.
- **Gap**: Service loader edge cases are not covered.

### Risk Assessment

Security and events crates are well-protected against regressions. The logging crate presents a false sense of security with its high test count but low assertion rate. Cloud API interactions and database pool behavior under load are the most significant untested risk areas.

## Desired State

- Security crate maintains its excellent coverage as new features are added.
- Events crate reaches 90%+ coverage with additional edge case tests.
- Database crate adds integration tests for connection pool behavior (timeouts, reconnection, pool exhaustion).
- Config crate adds edge case tests for schema validation.
- Cloud crate reaches 80%+ coverage with API interaction tests using mock HTTP clients.
- Logging crate's 275 no-op tests are either given meaningful assertions or removed and replaced with tests that verify behavior.
- Loader crate adds tests for service loader edge cases and error paths.
- All infrastructure crates maintain a minimum of 80% effective coverage (counting only tests with assertions).

## How to Get There

### Phase 1: Fix Logging Test Quality (Highest Priority)

1. Audit all 275 no-op tests in the logging crate to determine which represent real behavior that should be verified.
2. Add assertions to tests that construct meaningful objects (verify field values, format output, log levels).
3. Remove tests that exist solely to construct structs with no behavioral verification.
4. Add tests for log formatting, level filtering, and output routing that include meaningful assertions.

### Phase 2: Cloud API Coverage

1. Introduce mock HTTP client patterns for testing cloud API interactions without live services.
2. Write tests for tenant creation, authentication flows, and deployment operations against mocks.
3. Write tests for error handling when cloud API returns unexpected responses or timeouts.
4. Target 80% coverage for cloud API interaction logic.

### Phase 3: Database Integration Tests

1. Write integration tests for connection pool behavior: pool exhaustion, reconnection after failure, timeout handling.
2. Write tests for concurrent query execution to verify pool sharing behavior.
3. Write tests for transaction isolation and rollback behavior.

### Phase 4: Remaining Gaps

1. Add edge case tests for config schema validation (malformed YAML, missing required fields, type mismatches).
2. Add service loader tests for the loader crate covering discovery failures, duplicate modules, and circular dependencies.
3. Add event bus tests for high-volume scenarios and subscriber cleanup.

## Incremental Improvement Strategy

### Week 1-2: Logging Crate Quality Fix

Target: Audit and fix the 275 no-op tests. Either add assertions to make them meaningful or replace them with tests that verify actual behavior. This is the highest-impact change because it corrects a false sense of security. Expected result: logging effective coverage becomes accurately measurable; likely drops from 409 tests to approximately 200 tests with assertions.

### Week 3-4: Cloud API Tests

Target: 5 new test files with mock HTTP clients covering tenant management, authentication, and deployment flows. Expected result: cloud crate coverage rises from 65% to approximately 80%.

### Week 5-6: Database Pool and Config Edge Cases

Target: 3 new integration test files for database pool behavior, 2 new test files for config schema edge cases. Expected result: database coverage rises from 80% to approximately 85%; config coverage rises from 85% to approximately 90%.

### Week 7-8: Loader and Events Hardening

Target: 2 new test files for loader service edge cases, 2 new test files for events edge cases (high-volume, subscriber cleanup). Expected result: loader coverage rises from 70% to approximately 80%; events coverage rises from 85% to approximately 90%.

### Ongoing

Maintain the security crate's standard as the benchmark. Enforce a policy that infrastructure crate changes ship with tests that include assertions. Periodically audit test quality to prevent the no-op test pattern from recurring. Target 85%+ effective coverage across all infrastructure crates.
