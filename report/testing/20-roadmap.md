# 20 — Testing Roadmap & Metrics

This is the capstone report. It sequences all 19 preceding reports into an executable plan with dependency ordering, parallel work streams, and trackable metrics.

---

## Current State (2026-04-02)

**8,535 tests across 31 test crates. All passing. 0 failures.**

Phases 1-3 complete. Phase 4-5 coverage campaign (8 waves) complete. The codebase went from ~6,500 tests with many no-ops/trivials to 8,535 meaningful tests via an 8-wave parallel agent campaign.

### Per-Crate Test Counts

| Crate | Tests | Notes |
|-------|-------|-------|
| systemprompt-ai-tests | 843 | Converters, schema, structured output, tooled, providers |
| systemprompt-models-tests | 732 | A2A, API, artifacts, execution, validators, profile, AGUI, routing, modules |
| systemprompt-agent-tests | 684 | A2A server, orchestration, registry, skills, shared, MCP artifact |
| systemprompt-mcp-tests | 583 | UI renderer, CSP, schema, registry, orchestrator, process |
| systemprompt-oauth-tests | 548 | Auth provider, session, CIMD, WebAuthn, JWT, tokens |
| systemprompt-analytics-tests | 474 | Behavioral detection, metrics |
| systemprompt-api-tests | 429 | Middleware (158), routes/agent, routes/oauth, sync types |
| systemprompt-cloud-tests | 333 | Session store, API client |
| systemprompt-identifiers-tests | 328 | Email/URL/path/profile validation, all ID types overhauled |
| systemprompt-files-tests | 292 | File storage |
| systemprompt-content-tests | 289 | Builders, content types, links, serde |
| systemprompt-runtime-tests | 244 | Validation, config, database path |
| systemprompt-security-tests | 244 | JWT, auth types |
| systemprompt-traits-tests | 235 | Core interfaces |
| systemprompt-logging-tests | 226 | Models, CLI theme, trace (no-ops replaced with real assertions) |
| systemprompt-users-tests | 219 | User management |
| systemprompt-extension-tests (integration) | 201 | Integration tests |
| systemprompt-config-tests | 172 | Config loading |
| systemprompt-extension-unit-tests | 170 | **NEW** — metadata, errors, HList, builder, registry, typed extensions |
| systemprompt-templates-tests | 154 | Registry, embedded defaults |
| systemprompt-provider-contracts-tests | 147 | Provider traits |
| systemprompt-database-tests | 125 | Database abstraction |
| systemprompt-scheduler-tests | 123 | Job scheduling |
| systemprompt-sync-tests | 115 | Cloud sync |
| systemprompt-loader-tests | 114 | File/module discovery |
| systemprompt-generator-tests | 95 | Static site gen |
| systemprompt-events-tests | 81 | Event bus |
| systemprompt-client-tests | 78 | HTTP client |
| systemprompt-template-provider-tests | 39 | Template traits |

---

## Completed Phases

### Phase 1: Audit & Cleanup — COMPLETE

Baseline established. No-op tests identified. Coverage infrastructure configured (25.97% baseline).

### Phase 2: Infrastructure — COMPLETE

Coverage tooling configured. Mock types available. Integration test fixes done.

### Phase 3: Security-Critical Coverage — COMPLETE (2026-04-02)

Added 213+ tests across OAuth and API OAuth types. OAuth coverage: 34.1% → 42.8%.

### Phase 4-5: Coverage Campaign — COMPLETE (2026-04-02)

Executed as an 8-wave parallel agent campaign, 3 agents per wave. Each wave: write tests → verify build → commit → push.

| Wave | Commit | Tests After | Net Δ | Key Areas |
|------|--------|-------------|-------|-----------|
| 1 | c8ea01c14 | 6,897 | +325 | Logging no-op fixes, models artifacts/API/execution/validators |
| 2 | 57fbe1742 | 7,224 | +327 | Models profile/AGUI, AI converters, AI schema/structured output |
| 3 | d6aaab9b2 | 7,285 | +61 | Agent A2A errors/task builders, MCP artifact transformer, models/skills |
| 4 | 1398f02c1 | 7,806 | +521 | MCP UI renderer/CSP/schema/registry, API middleware (158 tests) |
| 5 | 3ab2a5ca5 | 8,086 | +280 | Cloud session store, extension system (0→170), app runtime |
| 6 | 43274b84a | 8,309 | +223 | MCP orchestrator/process, agent orchestration, AI tooled/tools |
| 7 | bf1c48bc1 | 8,442 | +133 | AI config/provider factory, agent registry/security, API routes/OAuth |
| 8 | 62a25d615 | 8,535 | +93 | Content/templates, identifiers overhaul, routing/modules sweep |

**Campaign totals:**
- 1,963 net new meaningful tests added
- ~320 trivial identifier tests replaced with ~250 focused validation/boundary tests
- ~275 logging no-op tests replaced with real assertion tests
- Extension system went from 0 → 170 unit tests (new crate created)
- 24 parallel agents across 8 waves
- Minimal production code changes (only `pub(crate)` → `pub` visibility bumps)

---

## Remaining Phases

### Phase 5b: Integration Test Improvements (Deferred)

DB-dependent repository tests and integration test database infrastructure. Not attempted in the campaign because the project rules prohibit mocking repositories/DI/DB.

| Area | Task | Effort |
|------|------|--------|
| OAuth session flows | Integration tests with real DB | 1 week |
| Agent DB operations | Repository integration tests | 1 week |
| MCP server lifecycle | End-to-end orchestration tests | 1 week |

### Phase 6: Advanced Testing (Not Started)

| Report | Task | Effort | Depends On |
|--------|------|--------|-----------|
| 19 | proptest for serialization, cargo-fuzz for parsers, criterion benchmarks, k6 load tests, protocol conformance | Ongoing | Coverage baseline |

### Explicitly Skipped

1. **CLI crate** (56.7K LOC, 1.8% coverage) — too large; needs dedicated E2E testing infrastructure
2. **DB-dependent repository tests** — need integration test DB; project rules prohibit mock DB
3. **WebSocket/streaming tests** — need runtime infrastructure
4. **Property-based/fuzz testing** — Phase 6, not in scope for this campaign

---

## Dependency Graph

```
PHASE 1: Audit & Cleanup                          ✅ COMPLETE
│
├── 01  Baseline Audit                             ✅
├── 02  File Quality & Standards                   ✅
├── 03  Trivial Test Cleanup                       ✅
├── 04  Assertion Quality Fixes                    ✅
└── 05  Error Path Gap Analysis                    ✅
         │
         ▼
PHASE 2: Infrastructure                            ✅ COMPLETE
│
├── 06  Mock Infrastructure                        ✅
├── 07  Coverage Measurement                       ✅ (25.97% baseline)
└── 08  Integration Test Fixes                     ✅
         │
         ▼
PHASE 3: Security-Critical Coverage                ✅ COMPLETE
│
├── 09  OAuth Coverage                             ✅ (42.8%)
└── 10  API Middleware Coverage                     ✅ (429 API tests)
         │
         ▼
PHASE 4-5: Coverage Campaign (8 waves)             ✅ COMPLETE
│
├── 11  Agent (A2A) Coverage                       ✅ (684 tests)
├── 12  AI Provider Coverage                       ✅ (843 tests)
├── 13  MCP Lifecycle Coverage                     ✅ (583 tests)
├── 14  CLI Coverage                               ⏭️  SKIPPED (needs E2E infra)
├── 15  Domain: Content/Files/Analytics             ✅ (289+292+474 tests)
├── 16  Infrastructure Improvements                ✅ (logging, cloud, events, config)
├── 17  App Layer Improvements                     ✅ (runtime 244 tests)
└── 18  Shared Layer Cleanup                       ✅ (identifiers overhauled, extension 170)
         │
         ▼
PHASE 6: Advanced Testing                          🔲 NOT STARTED
│
└── 19  Property-Based, Fuzz, Load, Contract
```

## Metrics

| Metric | Pre-Campaign | Post-Campaign (Now) | Target |
|--------|-------------|---------------------|--------|
| Total tests | 6,572 | 8,535 | — |
| Test crates | 30 | 31 (+extension-unit) | — |
| Extension unit tests | 0 | 170 | — |
| Identifier tests (meaningful) | ~320 (trivial) | 328 (validated) | — |
| Logging no-op tests | ~275 | 0 (all have assertions) | 0 |
| AI tests | ~577 | 843 | — |
| Agent tests | ~361 | 684 | — |
| MCP tests | ~297 | 583 | — |
| API tests | ~204 | 429 | — |
| Models tests | ~246 | 732 | — |
| Line coverage (overall) | 25.97% | TBD (run coverage) | 40%+ |
| Line coverage (OAuth) | 42.8% | TBD | 50%+ |
| Tests with assertions | ~76% | ~99%+ | 100% |
| Production code changes | — | Visibility bumps only | Minimal |

## What Changed in Production Code

The campaign made only minimal production code changes — exclusively `pub(crate)` → `pub` visibility bumps so test crates could access pure functions:

- `crates/domain/agent/src/services/a2a_server/processing/task_builder/mod.rs` — `pub mod helpers`
- `crates/domain/agent/src/services/mcp/artifact_transformer/mod.rs` — `pub mod` metadata/parts builders
- `crates/domain/agent/src/models/web/mod.rs` — re-export `extract_port_from_url`, `is_valid_version`
- `crates/domain/agent/src/services/registry/mod.rs` — `pub mod security`, `pub mod skills`
- `crates/domain/agent/src/services/registry/skills.rs` — `pub fn extract_description`, `pub struct SkillConfig`
- `crates/domain/mcp/src/services/ui_renderer/templates/mod.rs` — `pub mod html`
- `crates/domain/ai/src/services/tools/mod.rs` — re-export `request_to_tool_call`
- `crates/entry/api/src/services/middleware/` — pub on `is_datacenter_ip`, `is_known_bot`, etc.
- `crates/entry/api/src/routes/agent/` — pub on `create_mcp_extensions_from_config`, filter params
- `crates/app/runtime/src/validation.rs` — `pub fn validate_database_path`
