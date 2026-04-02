# Domain: Agent Crate Coverage

## Current State

**Source code:** 158 source files in `crates/domain/agent/src/` (~11,000+ lines)
**Test code:** 22 test files in `crates/tests/unit/domain/agent/src/` (418 tests)
**Coverage:** 13% of source files have corresponding tests

### What IS Tested (10 files)

- **Models:** A2A message serialization, AgentInfo, Context, Skills (10 test files -- mostly serialization round-trips and field validation)
- **Services:** `agent_orchestration` shared utilities (`auth.rs`, `resilience.rs`) -- 4 test files
- **Test breakdown:** 402 sync tests, 16 async tests

### What is NOT Tested (148+ files)

| Area | Files/LOC | Gap Description |
|------|-----------|-----------------|
| a2a_server handlers | 28 handler/processor files (~3,600 LOC) | ZERO tests. Request validation, message processing, artifact management, tool execution -- all untested. |
| a2a_server/streaming | ~1,300 LOC | ZERO tests. `webhook_client.rs`, `event_loop.rs`, `broadcast.rs` -- WebSocket and event handling completely uncovered. |
| a2a_server/processing | Message handling pipeline | ZERO tests. |
| AI execution strategy | Strategy selection logic | ZERO tests. |
| Artifact publishing | Artifact publishing service | ZERO tests. |
| External webhooks | Webhook integrations | ZERO tests. |
| Context/message persistence | Persistence layer | ZERO tests. |
| Tool execution paths | `plan_executor`, `tool_executor`, strategy selector | ZERO tests. |
| MCP integration | `artifact_transformer`, `tool_result_handler` | ZERO tests. |
| All repositories | Database repository implementations | ZERO tests. |

### Integration Tests

- 12 A2A protocol tests + 20 agent lifecycle tests = 32 total
- These cover basic task creation and state transitions but not error handling or edge cases

### Risk Assessment

This is the core product -- agents handling user requests via the A2A protocol. The entire request handling pipeline (from incoming request through tool execution to response generation) is untested. A regression in any of the 28 handler files would go undetected until production.

---

## Desired State

- Every handler/processor in `a2a_server/` has unit tests covering: valid input, invalid input, error propagation, and edge cases
- Streaming and WebSocket event handling has tests for: connection lifecycle, reconnection, message ordering, and backpressure
- Tool execution paths have tests for: strategy selection, execution success/failure, timeout handling, and result transformation
- MCP integration points have tests for: artifact transformation correctness, tool result mapping, and error handling
- Repository implementations have tests against an in-memory or test database
- Integration tests cover error paths and edge cases, not just happy paths
- Target: 60%+ source file coverage (95+ files with corresponding tests)

---

## How to Get There

### Phase 1: Handler Unit Tests (Highest Impact)

1. Create test files for each of the 28 `a2a_server` handler/processor files
2. Mock dependencies (database, AI service, MCP client) using trait objects or test doubles
3. Test request validation: malformed input, missing fields, invalid types
4. Test message processing: correct routing, state transitions, error responses
5. Test artifact management: creation, update, retrieval, deletion paths

### Phase 2: Tool Execution and Strategy Tests

1. Test `plan_executor`: plan parsing, step sequencing, failure recovery
2. Test `tool_executor`: tool invocation, result handling, timeout behavior
3. Test strategy selector: correct strategy chosen for different input types
4. Test MCP integration: `artifact_transformer` correctness, `tool_result_handler` mapping

### Phase 3: Streaming and Event Tests

1. Test `event_loop.rs`: event dispatch, ordering guarantees, shutdown behavior
2. Test `broadcast.rs`: subscriber management, message fan-out, slow consumer handling
3. Test `webhook_client.rs`: HTTP call construction, retry logic, failure handling

### Phase 4: Repository and Persistence Tests

1. Add repository tests using SQLx test fixtures or an in-memory database
2. Test context/message persistence: CRUD operations, constraint validation, concurrent access

### Phase 5: Integration Test Expansion

1. Add error path integration tests: invalid auth, malformed A2A messages, provider failures
2. Add edge case tests: concurrent task updates, message ordering under load, partial failures

---

## Incremental Improvement Strategy

### Week 1-2: Foundation
- Set up shared test utilities and mocks for the agent crate (AI service mock, database mock, MCP client mock)
- Write tests for the 5 most critical handler files (task creation, message send, status query, artifact retrieval, error handling)
- Target: 30 new tests, coverage moves from 13% to 18%

### Week 3-4: Tool Execution
- Write tests for `plan_executor`, `tool_executor`, and strategy selector
- Write tests for MCP integration points (`artifact_transformer`, `tool_result_handler`)
- Target: 40 new tests, coverage moves to 25%

### Week 5-6: Streaming and Events
- Write tests for `event_loop.rs`, `broadcast.rs`, `webhook_client.rs`
- Add async test infrastructure for WebSocket simulation
- Target: 30 new tests, coverage moves to 32%

### Week 7-8: Remaining Handlers and Repositories
- Complete handler test coverage for remaining 23 handler files
- Add repository tests with test database fixtures
- Target: 60 new tests, coverage moves to 45%

### Ongoing
- Every new handler or processor file must ship with tests (enforce via PR review)
- Add integration tests for each new A2A protocol feature
- Quarterly review of coverage gaps against production incident reports
