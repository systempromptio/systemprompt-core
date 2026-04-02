# Integration Test Strategy

## Current State

The project has 14 integration test crates in `crates/tests/integration/` with 549 total tests (279 sync, 270 async). Tests use real PostgreSQL database connections and benefit from well-designed test infrastructure including TestContext, SessionFactory, and TestCleanup with automatic fingerprint-based teardown.

### Integration Test Crates

| Crate | Tests | Notes |
|-------|-------|-------|
| extension | 222 | STRONGEST -- comprehensive trait and capability testing |
| users | 84 | All 84 have zero assertions -- effectively useless |
| files | 39 | All 39 have zero assertions -- effectively useless |
| analytics | 39 | 8 have no assertions |
| content | 30 | 22 have no assertions |
| models | 29 | |
| database | 23 | |
| agents | 20 | 5 have no assertions |
| scheduler | 17 | 3 have no assertions |
| oauth | 14 | Client lifecycle, tokens, webauthn -- these are GOOD |
| a2a | 12 | Basic protocol tests |
| auth | 11 | |
| common | 6 | |
| traits | 3 | |

### What Works Well

- **Extension integration tests** are excellent: 222 tests with comprehensive trait and capability testing across the extension framework.
- **OAuth integration tests** cover real flows: client CRUD, token exchange, PKCE validation.
- **TestContext/SessionFactory infrastructure** is well-designed, providing repeatable database setup and builder-pattern test data construction.
- **Database cleanup is automatic** via fingerprint tracking, preventing test pollution across runs.

### What Does Not Work

- **123 integration tests (22%) have zero assertions.** They execute code paths but verify no outcomes. These provide false confidence.
- **integration/users (84 tests)** and **integration/files (39 tests)** are completely hollow -- every single test lacks assertions.
- **No integration tests exist for:** AI pipeline execution, MCP server lifecycle, complete OAuth authorize-to-token flow, API endpoint handlers.
- **No performance or load testing infrastructure** exists at the integration level.

### Critical Missing Integration Tests

| Area | What Is Missing |
|------|-----------------|
| AI provider round-trips | Tests with mocked external APIs verifying prompt construction, response parsing, error handling |
| A2A full conversation flow | Create task, send messages, receive artifacts, complete/fail task lifecycle |
| MCP server lifecycle | Start server, discover tools, execute tool, handle errors, shutdown |
| OAuth full flow | Authorize, callback, token exchange, refresh, revocation |
| File operations | Upload, storage, retrieval, deletion with verification at each step |
| Content publish pipeline | End-to-end content creation through publishing |

## Desired State

- All 549 existing integration tests have meaningful assertions that verify outcomes, not just execution.
- Integration tests for every domain crate cover the primary success path and at least two error paths.
- AI, MCP, and A2A integration tests exist with mocked external dependencies (wiremock for HTTP, mock providers for AI).
- API endpoint integration tests use a test HTTP server to exercise real request/response cycles.
- Integration test execution completes in under 5 minutes on CI.
- Coverage of integration tests targets 60%+ line coverage for domain crates.

## How to Get There

### Step 1: Fix Hollow Tests

Audit every test with zero assertions. For each one, either:
- Add assertions that verify the expected outcome (return value, database state, side effects).
- Delete the test if it provides no value and cannot be meaningfully asserted.

Priority targets: `integration/users` (84 tests) and `integration/files` (39 tests) since every test in both crates is hollow.

### Step 2: Add Missing Domain Integration Tests

Write integration tests for the six critical missing areas listed above. Each test should:
- Set up preconditions via TestContext.
- Execute the operation under test.
- Assert on the return value AND on observable side effects (database state, events emitted).
- Clean up via the existing TestCleanup infrastructure.

### Step 3: Build API Integration Test Infrastructure

Create a test HTTP server harness that:
- Starts the API server on a random port.
- Provides a client for making requests with authentication.
- Supports asserting on response status, headers, and body.
- Tears down cleanly after each test.

### Step 4: Add Performance Baselines

Introduce basic performance integration tests using criterion or custom timing:
- Database query latency for common operations.
- API endpoint response time under single-user load.
- A2A message processing throughput.

## Incremental Improvement Strategy

**Week 1-2:** Fix the 123 hollow integration tests. Start with `integration/users` and `integration/files` since they are entirely assertion-free. Target: zero tests with no assertions.

**Week 3-4:** Write integration tests for the OAuth full flow and A2A conversation lifecycle. These are the most critical product paths. Target: 30 new integration tests with full assertions.

**Week 5-6:** Build the API integration test harness and write tests for the top 10 most-used API endpoints. Target: 50 new API integration tests.

**Week 7-8:** Add AI provider and MCP lifecycle integration tests using wiremock for external HTTP dependencies. Target: 40 new integration tests covering success and failure paths.

**Ongoing:** Every new feature or bug fix includes at least one integration test covering the primary path. Integration test count should grow by 10-20 per sprint.
