# Shared Layer Coverage

## Overview

The shared layer has 1,210 tests across 7 crates. Coverage is nominally high but inflated by trivial tests, particularly in the identifiers crate where ~400 tests verify derived trait implementations rather than meaningful behavior.

---

## Identifiers (crates/shared/identifiers/)

### Current State

- **Source**: 12 files (one per ID type + macros)
- **Tests**: 12 test files (450 tests)
- **File coverage**: 1:1 ratio -- looks perfect on paper
- **Problem**: ~420 of 450 tests are trivial (Clone, Debug, Eq, Hash, Display, Serialize, From, AsRef)
- **Effective coverage**: ~7% (only empty-check validation and format constraints are meaningful)
- **Detail**: Each ID type has ~35-40 tests, nearly all testing derived traits that the compiler already guarantees

### Desired State

- Test count drops from 450 to ~60 meaningful tests
- Each ID type has validation tests (empty string rejection, format constraints, length limits)
- Property-based tests verify round-trip serialization for all ID types
- No tests exist solely to verify derived trait implementations

### How to Get There

1. **Audit all 450 tests**: Identify which tests verify actual behavior (validation, format constraints) versus derived traits (Clone, Debug, Eq).
2. **Delete ~400 trivial tests**: Tests for Clone, Debug, Hash, Eq, Display, From, and AsRef on types that derive these traits provide no value. The compiler enforces these.
3. **Keep and strengthen validation tests**: Empty string rejection, format validation, length constraints -- these are the meaningful tests.
4. **Add proptest for round-trips**: A single proptest strategy per ID type replaces dozens of hand-written serialization tests with stronger guarantees. `proptest! { fn roundtrip(s in "[a-z0-9-]{1,64}") { let id = MyId::new(s.clone()); assert_eq!(id.to_string(), s); } }`
5. **Add boundary tests**: Maximum length IDs, minimum length IDs, IDs with special characters, unicode in IDs.

### Incremental Improvement Strategy

- **Week 1**: Delete trivial derived-trait tests. This cuts ~400 tests that add noise to CI output without providing signal.
- **Week 2**: Add proptest dependency and round-trip property tests for all 12 ID types.
- **Week 3**: Add boundary and edge case tests for each ID type's validation rules.

---

## Models (crates/shared/models/)

### Current State

- **Source**: ~15 files
- **Tests**: ~15 test files (210 tests)
- **File coverage**: Good 1:1 ratio
- **Quality**: Mix of serialization tests (useful for API contract stability) and structural tests (less useful)
- **Detail**: Config model tests are meaningful and verify real parsing behavior. Identity model tests are largely trivial structural checks.

### Desired State

- Serialization tests retained and strengthened as API contract guarantees
- Structural tests replaced with property-based tests where applicable
- All model types have deserialization-from-invalid-input tests to verify error messages
- Config model tests extended to cover all configuration variants and edge cases

### How to Get There

1. **Categorize existing tests**: Separate serialization contract tests (keep) from structural tests (evaluate).
2. **Strengthen serialization tests**: Add tests that verify backward compatibility -- deserialize old JSON formats into current model types.
3. **Add invalid input tests**: For each model that accepts external input, test deserialization with missing fields, extra fields, wrong types, and null values.
4. **Add config edge cases**: Test config parsing with missing optional fields, environment-specific overrides, and malformed YAML.

### Incremental Improvement Strategy

- **Week 1**: Add invalid input deserialization tests for all externally-facing model types.
- **Week 2**: Add backward compatibility tests for any models used in stored data or API responses.
- **Week 3**: Strengthen config model tests with edge cases and error scenarios.

---

## Traits (crates/shared/traits/)

### Current State

- **Source**: ~10 files
- **Tests**: ~10 test files (262 tests -- 240 sync, 22 async)
- **Coverage**: Good
- **Quality**: Trait contract tests are inherently valuable -- they verify that implementations satisfy interface requirements
- **Detail**: The async tests verify that async trait implementations work correctly across await boundaries

### Desired State

- Every trait has a contract test that can be run against any implementation
- Async traits tested for cancellation safety and concurrent execution
- Trait object safety verified where applicable
- Error contract tests verify that implementations return correct error types

### How to Get There

1. **Audit trait contract completeness**: Ensure every public trait method has at least one contract test.
2. **Add cancellation safety tests**: For async traits, test that dropping a future mid-execution doesn't corrupt state.
3. **Add concurrent execution tests**: For async traits used in concurrent contexts, verify correct behavior under parallel execution.
4. **Add error contract tests**: Verify that trait implementations return the documented error types for each failure mode.

### Incremental Improvement Strategy

- **Week 1**: Audit trait contract completeness and add missing method tests.
- **Week 2**: Add async cancellation and concurrency tests for the most critical traits.
- **Ongoing**: As new traits are added, require contract tests as part of the definition.

---

## Provider Contracts (crates/shared/provider-contracts/)

### Current State

- **Source**: ~8 files
- **Tests**: ~8 test files (168 tests)
- **Coverage**: Good
- **Quality**: Component renderer, job submission, schema extension traits all tested

### Desired State

- Every provider contract has a mock implementation with full contract test coverage
- Error handling contracts tested (what happens when a provider fails)
- Provider discovery and registration tested
- Contract backward compatibility verified across versions

### How to Get There

1. **Create mock implementations for every provider trait**: These serve as both test infrastructure and documentation of expected behavior.
2. **Add error handling contract tests**: Test that provider failures produce correct error types and don't corrupt caller state.
3. **Add contract versioning tests**: Verify that adding new methods with defaults doesn't break existing implementations.

### Incremental Improvement Strategy

- **Week 1**: Create mock implementations for any provider traits that lack them.
- **Week 2**: Add error handling and failure mode tests.
- **Ongoing**: Require contract tests for any new provider traits.

---

## Client (crates/shared/client/)

### Current State

- **Source**: ~5 files
- **Tests**: ~5 test files (78 tests -- 38 sync, 40 async)
- **Coverage**: Good
- **Quality**: HTTP client tests use async properly
- **Gap**: Error scenarios (network failures, timeouts, invalid responses) are undertested

### Desired State

- Error scenarios fully tested: network failures, connection timeouts, read timeouts, invalid response bodies, unexpected status codes
- Retry logic tested with configurable backoff
- Request/response logging tested for sensitive data redaction
- Connection pool behavior tested under load

### How to Get There

1. **Add network failure tests**: Use a mock HTTP server (wiremock-rs or similar) to simulate connection refused, connection reset, DNS failure.
2. **Add timeout tests**: Test both connection timeouts and read timeouts with slow mock servers.
3. **Add invalid response tests**: Test deserialization failures, truncated responses, and unexpected content types.
4. **Add retry logic tests**: Verify retry count, backoff timing, and retry-after header handling.

### Incremental Improvement Strategy

- **Week 1**: Add wiremock-rs dependency and network failure tests.
- **Week 2**: Add timeout and invalid response tests.
- **Week 3**: Add retry logic tests if retry behavior exists.

---

## Template Provider (crates/shared/template-provider/)

### Current State

- **Source**: ~3 files
- **Tests**: ~3 test files (42 tests -- 28 sync, 14 async)
- **Coverage**: Good

### Desired State

- Template loading error paths tested (missing files, permission errors, malformed templates)
- Async template loading tested for concurrent access patterns
- Template caching behavior tested if applicable

### How to Get There

1. **Add error path tests**: Test loading from non-existent paths, unreadable files, and syntactically invalid templates.
2. **Add concurrency tests**: Test concurrent template loading and verify no race conditions.

### Incremental Improvement Strategy

- **Week 1**: Add error path and edge case tests. This crate is small enough to reach near-complete coverage quickly.

---

## Extension (crates/shared/extension/)

### Current State

- **Source**: ~5 files
- **Tests**: Integration tests only (222 tests in integration/extension)
- **Coverage**: Excellent via integration tests
- **Quality**: Extension registration, discovery, capability composition all tested

### Desired State

- Unit tests added for internal logic to complement integration tests
- Extension conflict resolution tested (two extensions registering the same route/schema)
- Extension lifecycle tested (registration, initialization, teardown)
- Extension dependency resolution tested if applicable

### How to Get There

1. **Add conflict resolution tests**: Test what happens when two extensions register overlapping routes, duplicate schema names, or conflicting capabilities.
2. **Add lifecycle tests**: Test the full extension lifecycle from registration through teardown, including error handling at each stage.
3. **Add unit tests for internal helpers**: Any internal parsing or validation logic should have focused unit tests separate from integration tests.

### Incremental Improvement Strategy

- **Week 1**: Add conflict resolution tests -- these catch real bugs in multi-extension deployments.
- **Week 2**: Add lifecycle and internal helper unit tests.
- **Ongoing**: Maintain integration test quality as the extension framework evolves.
