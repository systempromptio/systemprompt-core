# Mock Infrastructure & Test Isolation

## Current State

The project lacks mock implementations for most external dependencies. Tests either use real database connections or skip database-dependent code entirely. The `wiremock` crate is available as a dependency but is rarely used. Test isolation is poor: unit tests cannot run without a database, error paths are largely untestable, and failure simulation is not possible.

### What Exists

| Component | Location | Quality |
|-----------|----------|---------|
| MockComponent | `crates/tests/unit/domain/templates/src/mocks.rs` | Good example of mock pattern |
| MockPageProvider | `crates/tests/unit/domain/templates/src/mocks.rs` | Good example of mock pattern |
| MockTemplateLoader | `crates/tests/unit/domain/templates/src/mocks.rs` | Good example of mock pattern |
| TestContext | Integration test infrastructure | Well-designed database setup |
| SessionFactory | Integration test infrastructure | Builder pattern for test data |
| TestCleanup | Integration test infrastructure | Automatic teardown via fingerprint |
| wiremock dependency | Available in Cargo.toml | Rarely used |

### What Is Missing

| Mock | Impact |
|------|--------|
| MockDbPool | Cannot test service logic without a real database. Every service test requires PostgreSQL. |
| MockAiProvider | Cannot test AI service error paths, rate limiting, malformed responses, timeout handling. |
| MockHttpClient | Cannot test HTTP client failures in MCP, A2A, and OAuth client code. |
| MockFileSystem | Cannot test disk failures, permission errors, storage quota exhaustion. |
| MockEventBus | Cannot test event-driven flows in isolation. Cannot verify events were emitted correctly. |
| MockScheduler | Cannot test job scheduling logic without running the full scheduler. |
| MockMcpServer | Cannot test MCP client behavior against controlled server responses. |

### Impact of Missing Mocks

- **Unit tests cannot isolate the code under test.** Service tests are actually integration tests because they require a real database.
- **Error paths and failure scenarios are untestable.** There is no way to simulate a database connection failure, a network timeout, or a malformed AI response.
- **Tests are slow.** Real database round-trips add 10-50ms per test. With thousands of tests, this compounds.
- **Tests are flaky.** Dependency on external state (database contents, network availability) introduces non-determinism.
- **Cannot simulate:** network timeouts, partial failures, concurrent access races, resource exhaustion, rate limiting, malformed external responses.

## Desired State

- Every external dependency has a corresponding mock implementation behind a trait boundary.
- Unit tests use mocks exclusively for isolation, testing only the logic of the unit under test.
- Integration tests use real implementations for end-to-end verification.
- Mock implementations support configurable responses, call counting, and common failure mode simulation.
- The mock infrastructure follows consistent patterns (MockBuilder) across all mock types.
- New external dependencies automatically get a mock implementation as part of the development process.

## How to Get There

### Step 1: Define Trait Interfaces

Ensure every external dependency is accessed through a trait interface. The traits that need to exist or be verified:

- `DatabasePool` trait -- abstracts over `sqlx::PgPool`
- `AiProvider` trait -- abstracts over AI service HTTP calls
- `HttpClient` trait -- abstracts over `reqwest::Client`
- `FileStorage` trait -- abstracts over filesystem operations
- `EventBus` trait -- abstracts over event publishing
- `Scheduler` trait -- abstracts over job scheduling

Where traits already exist, verify they are used consistently. Where they do not exist, introduce them at the boundary between the service and the external dependency.

### Step 2: Create Mock Implementations

For each trait, create a mock implementation that:

- Stores a queue of responses to return.
- Records all calls made (method, arguments).
- Supports returning errors to simulate failure modes.
- Implements `Default` for zero-configuration use in simple tests.

Follow the existing pattern from `MockComponent`/`MockPageProvider`/`MockTemplateLoader` in the templates test crate.

### Step 3: Build MockBuilder Pattern

Create a `MockBuilder` for each mock that supports:

```rust
let mock_db = MockDbPool::builder()
    .on_query("SELECT * FROM users WHERE id = $1")
    .returns(vec![test_user.clone()])
    .on_query("INSERT INTO users")
    .returns_error(DatabaseError::ConnectionRefused)
    .build();
```

This pattern makes test setup declarative and readable.

### Step 4: Expand wiremock Usage

For HTTP-dependent code (AI providers, OAuth endpoints, webhook delivery), use `wiremock` to:

- Set up expected HTTP requests with response bodies.
- Verify that the correct requests were made.
- Simulate HTTP failures (timeouts, 5xx errors, malformed JSON).

### Step 5: Migrate Unit Tests

Gradually migrate existing unit tests from no-external-dependency (testing only pure functions) to mock-based testing (testing service logic with mocked dependencies). Priority order:

1. Security-critical services (auth, OAuth)
2. User-facing services (AI, content)
3. Infrastructure services (database, events)

## Incremental Improvement Strategy

**Week 1:** Create `MockDbPool` and `MockHttpClient`. These unblock the most test scenarios. Verify that the `DatabasePool` and `HttpClient` traits exist and are used at all service boundaries.

**Week 2:** Create `MockAiProvider` and `MockEventBus`. Write 10 example unit tests using these mocks to establish patterns for the team.

**Week 3:** Create `MockFileSystem` and `MockScheduler`. Build the `MockBuilder` pattern as a shared test utility in `crates/tests/common/`.

**Week 4-5:** Migrate the top 20 most important service methods to mock-based unit tests. Focus on methods that currently have no tests because they require external dependencies.

**Week 6+:** Expand wiremock usage for all HTTP-dependent code. Add mock-based tests for every new service method as part of the development workflow.

**Ongoing rule:** No new external dependency is introduced without a corresponding mock implementation and trait boundary.
