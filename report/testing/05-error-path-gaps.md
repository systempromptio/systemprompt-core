# Error Path & Edge Case Coverage

**Grade: D+**

Only 7.6% of tests verify error or failure scenarios. Edge case coverage is nearly absent for integer boundaries, resource exhaustion, concurrency conflicts, and timeout conditions. The codebase has strong error types (well-defined enums with specific variants) but the test suite rarely exercises the paths that produce them.

---

## Current State

### Error Path Testing Overview

> **Last updated**: 2026-04-01. Phase 1 strengthened error assertions but did not add new error-path tests.

| Metric | Value | Change from Baseline |
|--------|-------|---------------------|
| Total tests | ~7,209 | -699 |
| Tests verifying error/failure | ~567 (7.9%) | Ratio improved slightly due to fewer total tests |
| `is_err()` assertions | 360 | -39 (converted to stronger patterns) |
| `unwrap_err()` assertions | ~170 | +12 (from strengthening) |
| `expect_err()` assertions | ~20 | +10 (from strengthening) |
| Error variant matching via `matches!()` | ~750 | +7 (from strengthening) |
| `#[should_panic]` tests | 0 | -6 (removed in trivial cleanup) |

### Error Coverage by Crate

| Area | Error Assertions | Assessment |
|------|-----------------|------------|
| OAuth / Validation | 100+ | **Best in codebase** -- redirect_uri (21), oauth_params (28), token validation |
| Security / Extraction | 90+ | Strong -- cookie parsing (28), token extraction (33), header parsing (14) |
| Config / Validation | 80+ | Good -- validator (25), schema_validation (16) |
| MCP / Agent error types | 80+ | Moderate -- error.rs (55), status transitions (8) |
| Loader Services | 60+ | Moderate |
| Database | 50+ | Moderate -- error.rs (31) |
| AI Services | 40+ | Low relative to complexity |
| Analytics | 30+ | Low |
| API handlers | ~0 | **Missing** |
| CLI commands | ~0 | **Missing** |
| Auth middleware | ~0 | **Missing** |

### Edge Case Coverage

| Edge Case Category | Occurrences in Tests | Assessment |
|-------------------|---------------------|------------|
| Empty string `""` | Well-covered in validation/security | Good for input validation; many occurrences are test data construction, not boundary testing |
| `None` as input | 1,881 across 130 files | Moderate; many are structural setup, not deliberate null-input testing |
| Zero values (0, 0.0) | 2,553 occurrences | Mostly structural defaults, not boundary testing |
| Unicode / special chars | ~50 occurrences | Low; only a handful of tests use non-ASCII input |
| MAX constants (i64::MAX, u64::MAX) | 12 occurrences in 15 files | **Very low** |
| Integer overflow/underflow | Near zero | **Missing** |
| Large collection sizes | Not tested | **Missing** |
| Concurrent operation conflicts | Not tested | **Missing** |
| Resource exhaustion (disk, memory, connections) | Not tested | **Missing** |
| Timeout scenarios | Not tested | **Missing** |

---

## Desired State

- 20%+ of tests cover error paths (currently 7.6%)
- Every service function that returns `Result` has tests for at least its two most common error variants
- Boundary values tested for all numeric parameters (0, 1, MAX, MAX+1 where applicable)
- Timeout behavior tested for all network-calling services
- Concurrent access tested for all shared-state services
- Every error enum variant is exercised by at least one test
- Edge case categories (empty input, null input, oversized input, malformed input) covered for all public API entry points

---

## How to Get There

### 1. Audit Error Enum Coverage

For each error enum in the codebase, verify that every variant is exercised by at least one test. Start with the most critical:

- `AuthError` variants (token expired, invalid signature, missing claims, revoked)
- `OAuthError` variants (invalid grant, invalid client, invalid scope, expired code)
- `DatabaseError` variants (connection failed, constraint violation, timeout, not found)
- `A2AError` variants (invalid message, unknown task, state transition violation)
- `McpError` variants (server unavailable, tool not found, execution failed)
- `ConfigError` variants (missing field, invalid value, file not found)

### 2. Add Boundary Value Tests

For every function that accepts numeric parameters:

```rust
#[test]
fn test_port_validation_boundaries() {
    assert!(validate_port(0).is_err());       // below minimum
    assert!(validate_port(1).is_ok());        // minimum valid
    assert!(validate_port(65535).is_ok());    // maximum valid
    assert!(validate_port(65536).is_err());   // above maximum
    assert!(validate_port(u32::MAX).is_err()); // extreme value
}
```

Priority targets: port numbers, page sizes, timeout durations, retry counts, file size limits, token expiration times.

### 3. Add Timeout and Network Failure Tests

Use mock HTTP clients to simulate:
- Connection refused
- Connection timeout (slow response)
- Partial response (connection dropped mid-stream)
- Invalid response body (malformed JSON)
- HTTP 429 (rate limited)
- HTTP 500/502/503 (server errors)

Priority targets: AI provider calls, MCP server communication, A2A protocol exchanges, OAuth token endpoints, cloud sync operations.

### 4. Add Concurrent Access Tests

Use `tokio::spawn` with shared state to verify:
- Two concurrent task state transitions on the same task
- Concurrent user creation with the same email
- Concurrent file uploads to the same path
- Concurrent session creation and revocation

### 5. Add Input Validation Edge Cases

For every public API endpoint, test:
- Empty body
- Oversized body (beyond configured limit)
- Malformed JSON (missing required fields, wrong types)
- SQL injection attempts in string fields
- Path traversal attempts in file paths
- Extremely long strings (1MB+)

---

## Incremental Improvement Strategy

**Week 1**: Catalog all error enums in the codebase and check which variants have test coverage. Produce a coverage matrix. This does not require writing tests -- it establishes the baseline and identifies the gaps.

**Week 2**: Write error path tests for the OAuth and Auth crates. These are security-critical and already have the best existing error tests to use as templates. Target: every `AuthError` and `OAuthError` variant exercised. This should add 30-50 tests.

**Week 3**: Write boundary value tests for config validation (ports, timeouts, sizes) and identifier validation (empty, too long, invalid characters). These are easy to write and catch real bugs. Target: 40-60 tests.

**Week 4**: Add timeout and connection failure tests for one network-calling service (start with AI provider or MCP client). This requires setting up mock infrastructure (`wiremock` is already a test dependency) but establishes the pattern for all other network services.

**Month 2**: Extend network failure tests to remaining services: A2A handlers, OAuth token exchange, cloud sync. Add concurrent access tests for the task state machine and user session management. Target: error path ratio above 12%.

**Month 3**: Add input validation edge cases for all API endpoints. Add oversized input tests, malformed JSON tests, and path traversal tests. Target: error path ratio above 15%.

**Quarter 2**: Add resource exhaustion tests (connection pool exhaustion, disk space, memory limits). Add fuzz testing with `cargo-fuzz` for JSON-RPC parsing, OAuth parameter handling, and file upload parsing. Target: error path ratio above 20%.

**Tracking metric**: (tests with error/failure assertions) / (total tests). Current: 7.6%. Target: 20%+.
