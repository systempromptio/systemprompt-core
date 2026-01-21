# systemprompt-analytics Compliance

**Layer:** Domain
**Reviewed:** 2026-01-21
**Verdict:** NON-COMPLIANT

---

## Checklist

| Category | Status |
|----------|--------|
| Boundary Rules | ✅ |
| Required Structure | ✅ |
| Code Quality | ❌ |

---

## Violations

| File:Line | Violation | Category |
|-----------|-----------|----------|
| `src/services/behavioral_detector.rs` | 345 lines (limit: 300) | File Length |
| `src/repository/fingerprint.rs` | 366 lines (limit: 300) | File Length |
| `src/models/cli.rs` | 386 lines (limit: 300) | File Length |
| `src/repository/agents.rs` | 401 lines (limit: 300) | File Length |
| `src/repository/core_stats.rs` | 404 lines (limit: 300) | File Length |
| `src/services/extractor.rs` | 430 lines (limit: 300) | File Length |
| `src/repository/tools.rs` | 432 lines (limit: 300) | File Length |
| `src/repository/funnel.rs` | 520 lines (limit: 300) | File Length |
| `src/repository/session/mutations.rs` | 303 lines (limit: 300) | File Length |
| `src/models/cli.rs:5-7` | Section header comments | Inline Comments |
| `src/models/cli.rs:67-69` | Section header comments | Inline Comments |
| `src/models/cli.rs:102-104` | Section header comments | Inline Comments |
| `src/models/cli.rs:137-139` | Section header comments | Inline Comments |
| `src/models/cli.rs:202-204` | Section header comments | Inline Comments |
| `src/models/cli.rs:248-250` | Section header comments | Inline Comments |
| `src/models/cli.rs:280-282` | Section header comments | Inline Comments |
| `src/models/cli.rs:309-311` | Section header comments | Inline Comments |
| `src/models/cli.rs:344-346` | Section header comments | Inline Comments |
| `src/repository/tools.rs:88` | Inline comment | Inline Comments |
| `src/repository/tools.rs:167` | Inline comment | Inline Comments |
| `src/repository/agents.rs:30-31` | Inline comment | Inline Comments |
| `src/repository/agents.rs:117` | Inline comment | Inline Comments |
| `src/services/anomaly_detection.rs:194` | `unwrap_or_default()` | Anti-Pattern |
| `src/services/extractor.rs:250` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/funnel.rs:202` | `unwrap_or_default()` | Anti-Pattern |
| `src/repository/fingerprint.rs:33` | `unwrap_or_default()` | Anti-Pattern |

---

## Commands Run

```
cargo clippy -p systemprompt-analytics -- -D warnings  # PASS
cargo fmt -p systemprompt-analytics -- --check          # PASS
```

---

## Actions Required

1. **Split oversized files into modules:**
   - `src/repository/funnel.rs` (520 lines) - extract helper structs/impl to submodule
   - `src/repository/tools.rs` (432 lines) - split by query type
   - `src/services/extractor.rs` (430 lines) - extract BOT_KEYWORDS and parsing logic
   - `src/repository/core_stats.rs` (404 lines) - split by metric category
   - `src/repository/agents.rs` (401 lines) - split by query type
   - `src/models/cli.rs` (386 lines) - split into category-specific files
   - `src/repository/fingerprint.rs` (366 lines) - extract constants/helper methods
   - `src/services/behavioral_detector.rs` (345 lines) - extract scoring/thresholds modules
   - `src/repository/session/mutations.rs` (303 lines) - trim 3 lines

2. **Remove inline comments:**
   - Delete section header comments from `src/models/cli.rs`
   - Delete explanation comments from `src/repository/tools.rs:88,167`
   - Delete explanation comments from `src/repository/agents.rs:30-31,117`

3. **Replace `unwrap_or_default()` with explicit handling:**
   - `src/services/anomaly_detection.rs:194` - use `unwrap_or_else(Vec::new)`
   - `src/services/extractor.rs:250` - use explicit `HashMap::new()` fallback
   - `src/repository/funnel.rs:202` - use `unwrap_or_else(Vec::new)`
   - `src/repository/fingerprint.rs:33` - use explicit empty vec construction

---

## Boundary Rules (PASS)

- ✅ No entry layer imports (`systemprompt-api`, `systemprompt-tui`)
- ✅ No direct SQL in services (all SQL in repositories)
- ✅ Repository pattern followed (services use repositories)
- ✅ Business logic in domain services

## Required Structure (PASS)

- ✅ README.md exists
- ✅ status.md exists
- ✅ `src/error.rs` exists
- ✅ `src/repository/` directory exists
- ✅ `src/services/` directory exists

## File Structure

```
crates/domain/analytics/
├── Cargo.toml
├── README.md
├── status.md
├── schema/
│   ├── anomaly_thresholds.sql
│   ├── engagement_events.sql
│   ├── fingerprint_reputation.sql
│   ├── funnels.sql
│   ├── funnel_progress.sql
│   └── migrations/
│       ├── 001_engagement_not_null.sql
│       ├── 002_add_content_id.sql
│       ├── 003_backfill_content_id.sql
│       └── 004_add_funnels.sql
└── src/
    ├── lib.rs (44 lines)
    ├── error.rs (27 lines)
    ├── models/
    │   ├── mod.rs (236 lines)
    │   ├── cli.rs (386 lines) ❌
    │   ├── engagement.rs (61 lines)
    │   ├── events.rs (140 lines)
    │   ├── fingerprint.rs (61 lines)
    │   └── funnel.rs (85 lines)
    ├── repository/
    │   ├── mod.rs (39 lines)
    │   ├── agents.rs (401 lines) ❌
    │   ├── cli_sessions.rs (143 lines)
    │   ├── content_analytics.rs (99 lines)
    │   ├── conversations.rs (152 lines)
    │   ├── core_stats.rs (404 lines) ❌
    │   ├── costs.rs (174 lines)
    │   ├── engagement.rs (205 lines)
    │   ├── events.rs (177 lines)
    │   ├── fingerprint.rs (366 lines) ❌
    │   ├── funnel.rs (520 lines) ❌
    │   ├── overview.rs (152 lines)
    │   ├── queries.rs (137 lines)
    │   ├── requests.rs (194 lines)
    │   ├── tools.rs (432 lines) ❌
    │   ├── traffic.rs (158 lines)
    │   └── session/
    │       ├── mod.rs (174 lines)
    │       ├── mutations.rs (303 lines) ❌
    │       ├── queries.rs (239 lines)
    │       └── types.rs (56 lines)
    └── services/
        ├── mod.rs (19 lines)
        ├── anomaly_detection.rs (202 lines)
        ├── behavioral_detector.rs (345 lines) ❌
        ├── detection.rs (32 lines)
        ├── extractor.rs (430 lines) ❌
        ├── service.rs (148 lines)
        ├── session_cleanup.rs (22 lines)
        └── throttle.rs (117 lines)
```
