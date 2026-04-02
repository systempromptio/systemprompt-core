# Domain: MCP Crate Coverage

## Current State

**Source code:** 94 source files across 13 service modules in `crates/domain/mcp/src/`
**Test code:** 33 test files in `crates/tests/unit/domain/mcp/src/` (346 tests -- 339 sync, 7 async)
**Coverage:** 17% of service logic has corresponding tests

### What IS Tested (9 service areas)

| Area | Test Files | What They Cover |
|------|-----------|-----------------|
| client | 2 | HTTP client basics, types |
| monitoring | 3 | Health checks, status |
| network | 2 | Port manager, network manager |
| orchestrator | 3 | Event bus, events only -- NOT daemon logic |
| process | 3 | Monitor, cleanup, PID management |
| ui_renderer | 2 | CSP policies, template basics |

### What is NOT Tested (massive gaps)

| Area | Gap Description |
|------|-----------------|
| Orchestrator daemon logic | Reconciliation, service validation, startup -- 10+ files completely untested |
| Process spawner | Binary execution, subprocess management -- ZERO tests |
| Lifecycle management | Startup, shutdown, restart flows -- 4 files, ZERO tests |
| MCP server deployment | Server deployment pipeline -- ZERO tests |
| Schema validation | Schema validation and loading -- ZERO tests |
| Tool provider integration | Tool provider connection and invocation -- ZERO tests |
| Database sync | Database synchronization operations -- ZERO tests |
| UI rendering | Actual HTML generation (forms, tables, charts) -- 8+ files, ZERO tests |
| Port conflict resolution | Port allocation conflict handling -- ZERO tests |
| Proxy health monitoring | Proxy health check and recovery -- ZERO tests |

### Risk Assessment

MCP server lifecycle (start/stop/restart/health) is critical operational functionality. Bugs in process management cause server hangs or orphaned processes. The orchestrator daemon -- responsible for keeping MCP servers running and healthy -- has zero tests on its core reconciliation logic. A bug here means servers silently stop working or consume resources indefinitely.

---

## Desired State

- Orchestrator daemon logic has tests for: reconciliation loop correctness, service validation rules, startup sequencing, and failure recovery
- Process spawner has tests for: binary resolution, argument construction, environment setup, and subprocess lifecycle
- Lifecycle management has tests for: clean startup, graceful shutdown, restart with state preservation, and crash recovery
- Schema validation has tests for: valid schemas, invalid schemas, schema migration, and version compatibility
- UI rendering has tests for: correct HTML output, data binding, empty states, and error states
- Port conflict resolution has tests for: allocation, conflict detection, and recovery
- Target: 50%+ service logic coverage with emphasis on lifecycle and daemon behavior

---

## How to Get There

### Phase 1: Orchestrator Daemon Tests (Highest Impact)

1. Test reconciliation logic: desired state vs actual state diffing, action generation
2. Test service validation: valid configurations accepted, invalid configurations rejected with clear errors
3. Test startup sequencing: dependency ordering, parallel start where safe, sequential start where required
4. Test failure recovery: single server failure, cascade failure, recovery after transient errors

### Phase 2: Process Management Tests

1. Test process spawner: binary path resolution, argument construction, environment variable injection
2. Test subprocess lifecycle: start, monitor, stop, kill, cleanup
3. Test PID management: PID file creation, stale PID detection, PID file cleanup on shutdown
4. Test lifecycle flows: clean startup sequence, graceful shutdown with drain, forced restart, crash detection and restart

### Phase 3: Schema and Deployment Tests

1. Test schema validation: well-formed schemas pass, malformed schemas produce actionable errors
2. Test schema loading: file discovery, parsing, version detection
3. Test MCP server deployment: configuration generation, server initialization, readiness checks

### Phase 4: UI and Network Tests

1. Test HTML generation: correct output for forms, tables, charts against expected snapshots
2. Test empty and error states: graceful rendering when data is missing or invalid
3. Test port conflict resolution: allocation under contention, conflict detection, alternative port selection
4. Test proxy health monitoring: health check intervals, failure detection, alerting

---

## Incremental Improvement Strategy

### Week 1-2: Orchestrator Core
- Write tests for the reconciliation loop (desired vs actual state comparison, action generation)
- Write tests for service validation rules
- Target: 20 new tests, coverage moves from 17% to 24%

### Week 3-4: Process Lifecycle
- Write tests for process spawner and subprocess management
- Write tests for startup, shutdown, and restart flows
- Add crash recovery tests
- Target: 25 new tests, coverage moves to 32%

### Week 5-6: Schema and Deployment
- Write schema validation tests with valid and invalid fixture files
- Write deployment pipeline tests
- Target: 15 new tests, coverage moves to 38%

### Week 7-8: UI Rendering and Network
- Write snapshot tests for HTML generation (forms, tables, charts)
- Write port conflict and proxy health tests
- Target: 20 new tests, coverage moves to 45%

### Ongoing
- Every new MCP service module must ship with lifecycle tests (start, stop, restart, health check)
- Process management changes require corresponding test updates
- Quarterly chaos testing: simulate process crashes, port conflicts, and network failures to validate recovery paths
