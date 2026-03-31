# Code Quality & Architecture Review

Review date: 2026-03-31
Codebase version: 0.1.18 (commit 38dcc3f50)
Reviewer: Automated deep audit

---

## Architecture Assessment

### Strengths

The layered architecture (shared -> infra -> domain -> app -> entry) is genuinely enforced with no circular dependencies detected. The extension framework using the `inventory` crate provides compile-time registration without runtime reflection overhead. The facade crate adds real value through feature flags (`core`, `database`, `api`, `cli`, `full`). Error handling discipline is exceptional: zero `unwrap()`/`expect()` calls in production code across 2,035 source files, and 1,211 uses of `.context()`/`.with_context()` for error propagation.

---

## ARCH-001: Agent Domain is Monolithic

**Severity:** HIGH (maintainability)
**Category:** Architecture
**Location:** `crates/domain/agent/` (20,105 lines, 158 files)

### Description

The agent domain handles five distinct responsibilities in a single crate:
1. A2A protocol server (streaming, request handling)
2. Agent process orchestration (lifecycle, spawning, monitoring)
3. Task scheduling and execution
4. Message routing and state management
5. Database persistence layer

### Evidence

Largest files by complexity:

| File | Lines | Responsibility |
|------|-------|----------------|
| `port_manager.rs` | 327 | Port allocation and tracking |
| `process.rs` | 317 | Subprocess spawning, env setup |
| `initialization.rs` | 320 | A2A protocol initialization |
| `completion.rs` | 282 | Message completion handling |
| `event_loop.rs` | 279 | A2A streaming state machine |

### Impact

- High cognitive load for developers working in the agent domain
- Changes to process management risk breaking A2A protocol logic
- Testing individual concerns requires understanding the full domain
- Onboarding new developers to this area takes significantly longer

### Recommendation

Split into three sub-domains within the next 12 months:
- `agent-process` — lifecycle, spawning, monitoring, port management
- `a2a-protocol` — message parsing, streaming, state machine, event loop
- `agent-orchestration` — coordination, task routing, inter-agent communication

---

## ARCH-002: CLI Entry Point is Oversized

**Severity:** MEDIUM (maintainability)
**Category:** Architecture
**Location:** `crates/entry/cli/` (71,485 lines)

### Description

The CLI entry point handles 50+ commands across admin, cloud, plugins, analytics, and database operations. Individual command files contain business logic that should live in service modules.

### Evidence

| File | Lines | Content |
|------|-------|---------|
| `cloud/db.rs` | 458 | Database backup, restore, cleanup logic |
| `cloud/profile/create.rs` | 403 | Interactive profile creation with validation |
| `admin/setup/wizard.rs` | 388 | Full setup walkthrough with state management |

### Impact

- CLI commands cannot be reused from the API or other entry points
- Business logic is tightly coupled to terminal I/O
- Testing requires simulating CLI interaction rather than calling services directly

### Recommendation

Extract command logic into service modules in `crates/app/`. CLI should be a thin layer: parse arguments, call services, format output. Example: move `setup::wizard` logic to `systemprompt-config` crate.

---

## CONCURRENCY-001: Blocking Mutex in Async Context

**Severity:** HIGH
**Category:** Concurrency
**File:** `crates/domain/ai/src/services/providers/gemini/provider.rs:16`

### Description

The Gemini AI provider uses a `std::sync::Mutex` (blocking) for the `ToolNameMapper` in an async context. When a tokio task holds this mutex and another task awaits it, the tokio worker thread is blocked, preventing other tasks from making progress.

### Vulnerable Code

```rust
pub(crate) tool_mapper: Arc<Mutex<ToolNameMapper>>,
```

### Impact

Under concurrent AI requests, tokio worker threads stall waiting for the mutex. With the default 4-8 worker threads, even a few concurrent Gemini requests can degrade the entire server's responsiveness, affecting all endpoints.

### Recommendation

Replace with `tokio::sync::Mutex` if the mapper requires mutation, or `Arc<ToolNameMapper>` if it's read-only after initialization.

---

## CONCURRENCY-002: Spawned Processes Inherit Parent File Descriptors

**Severity:** HIGH
**Category:** Resource Management
**File:** `crates/domain/agent/src/services/agent_orchestration/process.rs`

### Description

When spawning agent sub-processes via `Command::spawn()`, the code does not set `close_on_exec` flags or otherwise prevent child processes from inheriting the parent's file descriptors. The parent process typically holds 100+ open FDs for database connections, event streams, log files, and network sockets.

### Impact

With 50 spawned agents, that is 5,000+ FD references to the parent's resources. This can:
- Exhaust the per-process FD limit (typically 1,024 or 4,096)
- Prevent the parent from cleanly releasing resources (children hold references)
- Allow children to accidentally read/write parent's open files or sockets
- Keep database connections alive even after the parent attempts to close them

### Recommendation

```rust
use std::os::unix::process::CommandExt;

command.pre_exec(|| {
    // Close inherited FDs except stdin/stdout/stderr
    for fd in 3..1024 {
        let _ = nix::unistd::close(fd);
    }
    Ok(())
})
```

---

## CONCURRENCY-003: Broadcast Channel Silently Drops Events

**Severity:** MEDIUM
**Category:** Concurrency / Reliability
**File:** `crates/infra/events/src/services/broadcaster.rs:37`

### Description

The event broadcaster uses a `tokio::sync::broadcast` channel with a fixed capacity. When subscribers are slow and the buffer fills, events are silently dropped with no notification to the sender or the slow subscriber.

### Vulnerable Code

```rust
pub fn new(capacity: usize) -> Self {
    let (tx, _rx) = tokio::sync::broadcast::channel(capacity);
    // ...
}
```

### Impact

Under high load (100+ concurrent subscribers with network delays), events are lost without any indication. Analytics data, state synchronization notifications, and real-time updates may be silently incomplete. No metrics track dropped events.

### Recommendation

Monitor send results and log/metric dropped events:

```rust
match tx.send(event) {
    Ok(receiver_count) => { /* success */ }
    Err(broadcast::error::SendError(event)) => {
        tracing::warn!(event_type = %event.event_type(), "Event dropped: broadcast buffer full");
        metrics::counter!("events.dropped").increment(1);
    }
}
```

For critical events (auth state changes, security alerts), consider a separate bounded queue with backpressure rather than broadcast semantics.

---

## CONCURRENCY-004: Unbounded Log Channel

**Severity:** MEDIUM
**Category:** Resource Management
**File:** `crates/infra/logging/src/services/output/mod.rs:9`

### Description

Log events are published to a `dyn LogEventPublisher` trait object via a global static. There is no backpressure mechanism — if the log consumer is slow (network write to aggregator, disk I/O contention), log events accumulate unbounded in memory.

### Impact

Under sustained high throughput (100+ req/sec with verbose logging), the log queue can grow to hundreds of megabytes, eventually causing OOM. This is especially dangerous during incident response when logging volume increases dramatically.

### Recommendation

Bound the log queue with a maximum capacity (e.g., 10,000 entries). When full, drop the oldest entries (not newest — recent errors are more valuable during debugging). Add a metric for dropped log entries so operators know when this occurs.

---

## RESOURCE-001: Orphaned Agent Processes on Server Shutdown

**Severity:** HIGH
**Category:** Resource Management / Operations
**File:** `crates/domain/agent/src/services/agent_orchestration/process.rs:148`

### Description

Agent processes are spawned as detached children using `std::mem::forget(child)`. When the parent server shuts down (gracefully or not), spawned agents continue running as orphans with no supervision.

### Vulnerable Code

```rust
std::mem::forget(child);  // Detach: parent doesn't wait for child
```

### Impact

- Orphaned agents continue consuming CPU, memory, and database connections after the server stops
- Restart of the server may fail due to port conflicts with still-running agents
- No mechanism to discover or terminate orphaned processes
- Database connections from orphans may hold locks, blocking migrations

### Recommendation

Maintain a registry of spawned process IDs. During graceful shutdown, send SIGTERM to all registered processes and wait (with timeout) for them to exit. On startup, check for and clean up stale processes from previous runs.

---

## ERROR-001: Inconsistent Error-to-HTTP Translation

**Severity:** MEDIUM
**Category:** Error Handling / API Design
**Location:** Multiple domain error types

### Description

Each domain defines its own error enum with its own HTTP status code mapping strategy:
- `OrchestrationError` in agent domain maps to HTTP 500/400
- `McpError` in MCP domain uses a different mapping strategy
- `OAuthError` in OAuth domain has its own approach

Route handlers must understand each domain's error semantics and perform pattern matching to produce HTTP responses.

### Impact

- Adding a new error variant in any domain requires updating route handlers
- Risk of inconsistent HTTP status codes for semantically equivalent errors
- Internal error details may leak to API consumers when mappings are incomplete

### Recommendation

Create a unified `ApiError` type in `crates/entry/api/` with standard HTTP semantics. Implement `From<DomainError>` for each domain error type. Derive HTTP status codes from `ApiError` variants, not from domain-specific errors.

```rust
pub enum ApiError {
    NotFound { resource: &'static str },
    Unauthorized { reason: String },
    BadRequest { message: String },
    Conflict { reason: String },
    Internal { source: anyhow::Error },
}

impl From<OrchestrationError> for ApiError { /* ... */ }
impl From<McpError> for ApiError { /* ... */ }
impl From<OAuthError> for ApiError { /* ... */ }
```

---

## QUALITY-001: 723 Clone Operations in Agent Domain

**Severity:** MEDIUM
**Category:** Performance / Code Quality
**Location:** `crates/domain/agent/` (across all files)

### Description

The agent domain contains 723 instances of `.clone()`, `.to_string()`, and `.to_owned()`. This suggests heavy use of owned `String` types where borrowed `&str` or `Cow<str>` would suffice, and data structures passed by value instead of by reference.

### Impact

In a system handling 100+ concurrent agent interactions, unnecessary cloning compounds memory pressure and GC-like allocation overhead. Messages and contexts are cloned at every layer boundary.

### Recommendation

Profile the agent domain for allocation hotspots using `dhat` or `heaptrack`. Introduce `Cow<'_, str>` for message parts that are usually borrowed. Use `&str` for context lookups. Benchmark before and after to quantify improvement.

---

## QUALITY-002: Magic Numbers Without Rationale

**Severity:** LOW
**Category:** Code Quality / Documentation
**File:** `crates/infra/cloud/src/constants.rs:21-27` and others

### Description

The codebase contains 50+ hardcoded constants with no documentation explaining why those specific values were chosen:

```rust
CALLBACK_TIMEOUT_SECS: u64 = 300    // Why not 180 or 600?
PROVISIONING_POLL_INTERVAL_MS: u64 = 2000  // Tuned for what latency?
```

### Impact

Developers modifying these values have no context for safe ranges. Values may have been tuned for specific production conditions that are no longer documented.

### Recommendation

Add doc comments to each constant explaining the rationale, measurement units, and any production tuning history. Consider making performance-sensitive values configurable via the profile system.

---

## TEST-001: Agent Domain Has Zero Unit Tests

**Severity:** HIGH
**Category:** Testing
**Location:** `crates/domain/agent/` (158 source files, 0 unit tests)

### Description

The most complex domain in the codebase — handling A2A protocol, process orchestration, task scheduling, message routing, and persistence — has zero unit tests. Only 8 integration tests exist for high-level flows.

### Evidence

| Domain | Source Files | Unit Tests | Integration Tests | Effective Coverage |
|--------|-------------|------------|-------------------|--------------------|
| agent | 158 | 0 | 8 | ~5% |
| ai | 98 | 0 | 1 | ~1% |
| mcp | 94 | 0 | 0 | 0% |
| oauth | 60 | 0 | 4 | ~7% |

The A2A event loop (`event_loop.rs`, 279 lines of state machine logic) is completely untested. Port manager allocation/deallocation logic is untested. Agent lifecycle transitions are untested.

### Impact

- Refactoring agent code is high-risk with no regression safety net
- Bug fixes must be tested manually
- State machine edge cases (message ordering, error recovery, timeout handling) are unverified
- New contributors cannot safely modify agent code

### Recommendation

Priority unit tests to add:
1. Port manager — allocation, deallocation, conflict detection
2. Agent lifecycle — state transitions (pending -> running -> failed -> stopped)
3. Event loop — message ordering, error recovery, cancellation
4. Process spawning — environment sanitization, argument construction
5. A2A message parsing — malformed input, edge cases

Target: 70% branch coverage for agent domain within 2 sprints.

---

## TEST-002: Integration Tests Are Happy-Path Only

**Severity:** MEDIUM
**Category:** Testing
**Location:** `crates/tests/integration/`

### Description

All existing integration tests verify success scenarios. No tests exercise failure paths: database connection loss mid-transaction, timeout during OAuth flow, resource exhaustion during file upload, concurrent access conflicts.

### Impact

Production failures are almost always edge cases. Without failure scenario testing, the first time these code paths execute is in production during an incident.

### Recommendation

Add failure scenario tests:
- Kill database connection mid-transaction; verify graceful recovery
- Inject latency into HTTP calls; verify timeout handling
- Exhaust memory during file upload; verify cleanup
- Concurrent OAuth flows for the same user; verify state consistency

---

## TEST-003: No Contract Tests Between Domains

**Severity:** MEDIUM
**Category:** Testing
**Location:** Cross-domain boundaries

### Description

The agent domain calls MCP repository methods. MCP calls the config system. Content calls the database layer. These cross-domain boundaries are not explicitly tested. If an upstream domain changes a return type or method signature, downstream domains break at runtime (or compile time if lucky).

### Impact

Refactoring any domain's public API risks silent breakage in dependent domains. The current approach relies entirely on the Rust compiler catching type mismatches, which doesn't cover semantic changes (e.g., a method now returns `None` where it previously always returned `Some`).

### Recommendation

Create contract tests that verify each domain's public API behaves as downstream consumers expect. These tests serve as an early warning system when cross-domain APIs change semantics.

---

## DEP-001: Multiple Crate Versions Allowed

**Severity:** LOW
**Category:** Dependencies
**Location:** `Cargo.toml` workspace lints

### Description

The workspace configuration sets `multiple_crate_versions = "allow"` in clippy lints. This permits the dependency tree to contain multiple versions of the same crate, which increases binary size and can cause type incompatibility issues.

### Recommendation

Change to `"warn"` and audit the dependency tree quarterly for version skew. Use `cargo tree -d` to identify duplicates.
