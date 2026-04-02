# 20 — Testing Roadmap & Metrics

This is the capstone report. It sequences all 19 preceding reports into an executable plan with dependency ordering, parallel work streams, and trackable metrics.

---

## Dependency Graph

```
PHASE 1: Audit & Cleanup (no dependencies — start here)
│
├── 01  Baseline Audit                    ─┐
├── 02  File Quality & Standards           │  All independent,
├── 03  Trivial Test Cleanup               │  can run in parallel
├── 04  Assertion Quality Fixes            │
└── 05  Error Path Gap Analysis           ─┘
         │
         ▼
PHASE 2: Infrastructure (depends on Phase 1 understanding)
│
├── 06  Mock Infrastructure               ─┐  Must complete before
├── 07  Coverage Measurement               │  any coverage work
└── 08  Integration Test Fixes            ─┘  (needs mocks from 06)
         │
         ▼
PHASE 3: Security-Critical Coverage (depends on mocks)
│
├── 09  OAuth Coverage                     ─┐  Security-first:
└── 10  API Middleware Coverage            ─┘  auth before features
         │
         ▼
PHASE 4: Core Product Coverage (depends on mocks)
│
├── 11  Agent (A2A) Coverage               ─┐
├── 12  AI Provider Coverage                │  Can run in parallel
└── 13  MCP Lifecycle Coverage             ─┘
         │
         ▼
PHASE 5: Remaining Coverage (depends on mocks, lower priority)
│
├── 14  CLI Coverage                       ─┐
├── 15  Domain: Content/Files/Analytics     │  Can run in parallel
├── 16  Infrastructure Improvements         │
├── 17  App Layer Improvements              │
└── 18  Shared Layer Cleanup               ─┘
         │
         ▼
PHASE 6: Advanced Testing (depends on coverage baseline)
│
└── 19  Property-Based, Fuzz, Load, Contract
```

## Execution Timeline

### Phase 1: Audit & Cleanup (Weeks 1-2)

All 5 tasks are independent and can run in parallel.

| Report | Task | Effort | Parallel? |
|--------|------|--------|-----------|
| 01 | Establish baseline metrics, document current state | 1 day | Yes |
| 02 | Split 6 files over 1,000 lines, fix 31 brittle assertions | 3 days | Yes |
| 03 | Delete ~3,000 trivial tests (Send/Sync, derive, no-assertion) | 3 days | Yes |
| 04 | Replace 512 weak `is_ok()` assertions with value inspection | 2 days | Yes |
| 05 | Catalogue all error-path gaps per crate, prioritize | 1 day | Yes |

**Expected outcome:** Test count drops from 7,908 to ~4,500-5,000. Every remaining test has meaningful assertions. Test suite runs faster. Signal-to-noise ratio jumps from ~60% to ~95%.

### Phase 2: Infrastructure (Weeks 3-5)

Mock infrastructure must complete before coverage work begins. Coverage measurement can start in parallel with mock creation.

| Report | Task | Effort | Depends On |
|--------|------|--------|-----------|
| 06 | Create MockDbPool, MockAiProvider, MockHttpClient, MockEventBus | 2 weeks | Phase 1 |
| 07 | Configure coverage tooling, generate first report, add to CI | 1 week | 01 |
| 08 | Fix 123 hollow integration tests (add assertions or delete) | 1 week | 06 |

**Expected outcome:** Service-layer unit tests can run without a database. Error paths become testable. Coverage numbers are visible for the first time.

### Phase 3: Security-Critical Coverage (Weeks 5-7) — COMPLETE

Completed 2026-04-02. Added 213 new tests across 12 files.

| Report | Status | Tests Added | Coverage Change |
|--------|--------|-------------|----------------|
| 09 | COMPLETE | 139 tests (auth_provider, session, CIMD, WebAuthn config/jwt/token/user_service/types) | OAuth: 34.1% → 42.8% |
| 10 | COMPLETE | 74 tests (authorize types, token types/errors, client_config, responses) | API OAuth types covered |

**Outcome:** OAuth service layer (auth providers, JWT validation, WebAuthn config/token/user) now has comprehensive unit tests. API OAuth public types tested. Remaining gaps are DB-dependent code (session flows, credential validation, full WebAuthn ceremonies) requiring integration tests.

### Phase 4: Core Product Coverage (Weeks 7-11)

These three crates are the core product. Can run in parallel with 3 developers.

| Report | Task | Effort | Depends On |
|--------|------|--------|-----------|
| 11 | A2A server handlers, message processing, tool execution (28+ files) | 3 weeks | 06 |
| 12 | AI provider integration, generation logic, streaming, error handling | 2 weeks | 06 |
| 13 | MCP orchestration, process lifecycle, server start/stop/health | 2 weeks | 06 |

**Expected outcome:** Core product code goes from 10-20% to 50-60% coverage. The most dangerous untested code paths are covered.

### Phase 5: Remaining Coverage (Weeks 11-15)

Lower priority but still valuable. All tasks are independent.

| Report | Task | Effort | Depends On |
|--------|------|--------|-----------|
| 14 | CLI command execution, argument parsing, output formatting | 2 weeks | 06 |
| 15 | Content provider, file upload, analytics behavioral detection | 2 weeks | 06 |
| 16 | Fix logging no-ops, cloud API tests, loader edge cases | 1 week | 06 |
| 17 | Sync conflict resolution, runtime lifecycle, scheduler concurrency | 1 week | 06 |
| 18 | Delete ~400 trivial identifier tests, add proptest round-trips | 1 week | Phase 1 |

**Expected outcome:** Full codebase achieves 40%+ line coverage. No crate has 0% coverage.

### Phase 6: Advanced Testing (Ongoing, from Week 8+)

Can begin once mock infrastructure exists. Runs indefinitely alongside regular development.

| Report | Task | Effort | Depends On |
|--------|------|--------|-----------|
| 19 | proptest for serialization, cargo-fuzz for parsers, criterion benchmarks, k6 load tests, protocol conformance | Ongoing | 06, 07 |

**Expected outcome:** Classes of bugs that hand-written tests miss (edge cases, performance regressions, protocol violations) are caught automatically.

## Current vs Target Testing Pyramid

### Before (Now)

```
                 /\
                /  \          Integration: 549 (7%)
               /    \         22% have no assertions
              /------\
             /        \       Unit: 7,359 (93%)
            /          \      38% trivial, 55% structural
           /______________\
```

### After Phase 1 (Cleanup)

```
                 /\
                /  \          Integration: ~450 (10%)
               /    \         Hollow tests deleted
              /------\
             /        \       Unit: ~4,500 (90%)
            /          \      All meaningful, 70%+ behavioral
           /______________\
```

### After Phase 5 (Full Coverage)

```
                 /\
                /  \          E2E/Load: ~50
               /    \         Full user flows, perf baselines
              /------\
             /        \       Integration: ~800
            /          \      Real DB, mocked externals, error paths
           /            \
          /---------- ---\
         /                \   Unit: ~5,000
        /                  \  80%+ behavioral, mock-isolated
       /                    \ Error paths, edge cases, boundaries
      /______________________\
```

## Metrics to Track

| Metric | Current | After Phase 1 | After Phase 3 (actual) | After Phase 5 |
|--------|---------|---------------|------------------------|---------------|
| Total tests | 7,908 | ~4,500 | 679 (OAuth 530 + API 149) | ~6,500 |
| Tests with assertions | 6,044 (76%) | ~4,500 (100%) | 679 (100%) | ~6,500 (100%) |
| Behavioral test ratio | ~45% | ~70% | ~75% | ~80% |
| Error path coverage | ~7.6% | ~10% | ~18% | ~22% |
| Line coverage (overall) | Unknown | 25.97% | 14.84% (grcov full) | ~55% |
| Line coverage (OAuth) | Unknown | 34.1% | 42.8% | ~60% |
| Line coverage (security) | Unknown | 96.8% | 96.8% (maintained) | ~80% |
| Files over 300 lines | 135 | ~80 | ~60 | ~30 |
| No-assertion tests | 1,864 | 0 | 0 | 0 |
| Trivial tests | ~3,029 | ~200 | ~200 | ~100 |
| Mock types available | 3 | 7+ | 7+ | 10+ |
| Test execution (unit) | Unknown | <45s | <60s | <90s |
| Test execution (integration) | Unknown | <3m | <4m | <5m |
