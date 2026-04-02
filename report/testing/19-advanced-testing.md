# Advanced Testing: Property-Based, Fuzz, Load, Contract

## Property-Based Testing

### Current State

- **Tests**: ZERO property-based tests anywhere in the codebase
- **Tools**: Neither `proptest` nor `quickcheck` appear in any Cargo.toml
- **Impact**: 534+ hand-written serialization/deserialization tests exist that could be replaced with stronger property-based guarantees
- **Risk**: Hand-written tests only cover the specific cases the author thought of, missing edge cases that property-based testing would find automatically

### Desired State

- `proptest` added as a dev-dependency to crates with model types
- Serialization round-trip properties verified for all model types: `serialize(x) |> deserialize == x`
- A2A task state machine properties verified: no sequence of valid transitions reaches an invalid state
- OAuth parameter combination properties verified: arbitrary valid/invalid parameters are handled correctly
- Identifier validation properties verified: arbitrary strings produce either a valid ID or a clean error, never a panic
- JSON schema transformation properties verified: arbitrary valid schemas produce valid provider-specific schemas

### How to Get There

1. **Add proptest to workspace dev-dependencies**: Single addition to the workspace Cargo.toml, available to all crates.
2. **Start with serialization round-trips**: These are the highest-value, lowest-effort property tests. For every type that implements both Serialize and Deserialize, add `proptest! { fn roundtrip(x: MyType) { assert_eq!(from_json(to_json(x.clone())), x); } }`. This requires implementing `Arbitrary` for each type, which proptest's `#[derive(Arbitrary)]` macro handles for most types.
3. **Add state machine properties**: Define the valid A2A task state transitions as a proptest strategy. Generate random sequences of transitions and verify the state machine never enters an invalid state.
4. **Add identifier validation properties**: Generate arbitrary strings (including empty, unicode, very long, null bytes) and verify that ID construction either succeeds with a valid ID or fails with a descriptive error.
5. **Add OAuth parameter properties**: Generate arbitrary combinations of OAuth parameters and verify the system handles them without panics.

### Incremental Improvement Strategy

- **Week 1**: Add proptest dependency. Write round-trip property tests for the 5 most critical model types (Task, Message, AgentCard, User, Config). This alone replaces ~100 hand-written serde tests.
- **Week 2**: Add state machine property tests for A2A task lifecycle and identifier validation properties.
- **Week 3**: Add OAuth parameter properties and JSON schema transformation properties.
- **Ongoing**: Require property tests for any new model types added to the codebase.

---

## Fuzz Testing

### Current State

- **Fuzz targets**: ZERO
- **Tools**: cargo-fuzz with libfuzzer or afl -- neither configured
- **Risk**: Parsing untrusted input (file uploads, JSON-RPC messages, OAuth parameters) without fuzz testing means crashes, hangs, and unexpected panics go undetected until production

### Desired State

- `cargo-fuzz` configured with a `fuzz/` directory at the workspace root
- 5+ fuzz targets covering all untrusted input parsing paths
- Fuzz targets run in CI on a schedule (nightly or weekly)
- Any crashes found are converted into regression tests

### How to Get There

Priority fuzz targets, ordered by security impact:

1. **File upload parsing**: Base64 decoding, MIME type detection, path construction. A malformed upload should never cause a panic or path traversal.
2. **JSON-RPC message deserialization**: A2A and MCP protocol messages arrive from untrusted sources. Malformed JSON-RPC should produce clean errors, not crashes.
3. **OAuth parameter handling**: Authorization URL parsing, redirect URI validation, state parameter handling. OAuth endpoints face the public internet.
4. **Configuration loading**: YAML parsing, secret file parsing. Malformed config should fail with clear errors during startup, not crash mid-operation.
5. **JSON deserialization for API endpoints**: All API endpoints accepting JSON bodies should handle malformed input gracefully.

Setup steps:
1. Run `cargo install cargo-fuzz` in CI and development environments.
2. Create `fuzz/Cargo.toml` with dependencies on the crates being fuzzed.
3. Create one fuzz target per priority item above.
4. Add a CI job that runs each fuzz target for a fixed duration (e.g., 60 seconds) on every PR, and for longer (e.g., 10 minutes) nightly.

### Incremental Improvement Strategy

- **Day 1**: Install cargo-fuzz and create the fuzz directory structure.
- **Week 1**: Add fuzz targets for file upload parsing and JSON-RPC deserialization (highest security impact).
- **Week 2**: Add fuzz targets for OAuth parameter handling and configuration loading.
- **Week 3**: Add the API endpoint JSON deserialization fuzz target and configure CI scheduling.
- **Ongoing**: Convert any fuzz-found crashes into permanent regression tests.

---

## Load/Performance Testing

### Current State

- **Benchmarks**: ZERO benchmarks anywhere in the codebase
- **Load tests**: ZERO load test configurations
- **Tools**: criterion (micro-benchmarks) and k6 (load testing) -- neither configured
- **Risk**: Performance characteristics are unknown. Design choices (pool sizes, lock strategies, serialization formats) are based on assumptions, not measurements.

### Desired State

- `criterion` benchmarks for hot-path operations (serialization, struct assembly, lock contention)
- `k6` load test suite for API endpoint throughput and latency under concurrent load
- Performance baselines established and tracked over time
- CI detects performance regressions before they reach production

### How to Get There

Criterion micro-benchmark candidates:
- **Task construction time**: Complex struct assembly for A2A tasks with multiple parts and artifacts
- **Event broadcast latency**: RwLock<HashMap> contention under concurrent reads and writes
- **JSON serialization/deserialization throughput**: Measure serde_json performance for the largest model types
- **Database query latency**: Measure query execution time under connection pool pressure
- **Template rendering throughput**: Measure rendering speed for complex templates with many partials

k6 load test candidates:
- **SSE connection scaling**: How many concurrent SSE/WebSocket connections can the server sustain before latency degrades
- **OAuth token issuance rate**: Token endpoint throughput under concurrent authentication requests
- **API endpoint latency percentiles**: p50, p95, p99 latency for each major API endpoint under load
- **File upload throughput**: Concurrent file upload handling with various file sizes
- **A2A task creation rate**: Concurrent task creation and status polling

### Incremental Improvement Strategy

- **Week 1**: Add criterion dependency and write benchmarks for JSON serialization and task construction. These establish baseline performance numbers.
- **Week 2**: Add benchmarks for event broadcast latency and template rendering. These validate lock strategy and rendering pipeline design choices.
- **Week 3**: Set up k6 and write load tests for the 3 most critical API endpoints (task creation, event streaming, authentication).
- **Week 4**: Add remaining k6 load tests and configure CI performance regression detection.
- **Ongoing**: Run benchmarks before and after any performance-sensitive change. Track results over time.

---

## Protocol Contract Testing

### Current State

- **Contract tests**: ZERO
- **Protocols implemented**: A2A (JSON-RPC based), MCP (JSON-RPC based), OAuth 2.0/OIDC, WebAuthn
- **Risk**: Protocol implementations may drift from specifications without detection, breaking interoperability with third-party clients and servers

### Desired State

- A2A protocol conformance tests verify compliance with the official Agent-to-Agent Protocol specification
- MCP protocol conformance tests verify compliance with the Model Context Protocol specification
- OAuth 2.0 conformance tests verify RFC 6749 compliance
- OIDC conformance tests verify OpenID Connect Core 1.0 compliance
- WebAuthn conformance tests verify WebAuthn Level 2 compliance
- All contract tests run in CI on every PR

### How to Get There

1. **A2A contract tests**: Parse the A2A protocol specification. For each required message type, verify that the implementation produces conformant JSON-RPC messages. For each required response type, verify correct deserialization and handling. Test error codes match the specification.
2. **MCP contract tests**: Follow the same approach for the MCP specification. Verify tool discovery, resource listing, and prompt handling produce spec-compliant messages.
3. **OAuth 2.0 contract tests**: Verify each grant type produces RFC 6749 compliant requests and responses. Test error responses match RFC 6749 error codes. Verify token formats and expiry handling.
4. **OIDC contract tests**: Verify ID token claims, userinfo endpoint responses, and discovery document format match the OIDC Core 1.0 specification.
5. **WebAuthn contract tests**: Verify credential creation and assertion ceremonies produce WebAuthn Level 2 compliant structures.

### Incremental Improvement Strategy

- **Weeks 1-2**: A2A protocol contract tests. This is the most critical protocol for the platform's core value proposition.
- **Weeks 3-4**: MCP protocol contract tests. This is the second most critical protocol for integration capabilities.
- **Weeks 5-6**: OAuth 2.0 and OIDC contract tests. These affect security and interoperability with identity providers.
- **Weeks 7-8**: WebAuthn contract tests.
- **Ongoing**: Update contract tests whenever protocol specifications are updated.

---

## Concurrency Testing

### Current State

- **Concurrency tests**: ZERO
- **Risk**: The platform is multi-tenant and handles concurrent users. Race conditions, deadlocks, and data corruption under concurrent access are entirely untested.

### Desired State

- Critical concurrent access paths tested with multiple simultaneous tasks
- Lock contention scenarios tested and verified to not deadlock
- Database connection pool exhaustion handled gracefully
- Concurrent file uploads tested for data integrity
- MCP server lifecycle tested for race conditions during startup/shutdown

### How to Get There

Candidates for concurrency testing:

1. **Concurrent A2A task creation**: Multiple agents creating tasks simultaneously. Verify no task ID collisions, no lost updates, and correct state transitions under contention.
2. **Concurrent event broadcast**: Multiple publishers and subscribers operating simultaneously. Verify no dropped events, no duplicate deliveries, and correct ordering guarantees.
3. **Database connection pool exhaustion**: Saturate the connection pool and verify that excess requests queue correctly, timeout appropriately, and produce clean errors rather than panics.
4. **Concurrent file uploads**: Multiple simultaneous uploads to the same tenant. Verify no file corruption, no path collisions, and correct metadata recording.
5. **MCP server lifecycle races**: Start and stop MCP servers concurrently. Verify no zombie processes, no port conflicts, and clean state after rapid start/stop cycles.

Tools:
- **loom**: For verifying lock-free data structures and low-level concurrency primitives. Best for testing the event broadcast system's internal lock strategy.
- **tokio::test with JoinSet**: For testing application-level concurrency. Spawn multiple tasks and verify correct behavior under parallel execution.
- **std::sync::Barrier**: For synchronizing test threads to maximize contention and expose race conditions.

### Incremental Improvement Strategy

- **Week 1**: Add concurrent A2A task creation tests using tokio::test with multiple spawned tasks. This is the highest-risk concurrent path.
- **Week 2**: Add concurrent event broadcast tests and database connection pool exhaustion tests.
- **Week 3**: Add concurrent file upload tests and MCP server lifecycle race tests.
- **Ongoing**: Any bug report involving concurrent access should result in a new concurrency test that reproduces the issue before fixing it.
