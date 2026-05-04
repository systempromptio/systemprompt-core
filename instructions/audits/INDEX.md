# systemprompt-core Audit Index

**Generated:** 2026-05-04
**Wave:** 1 baseline (concurrent fixes in flight by other Wave 1 agents)

This index lists every published crate in dependency-layer order. Counts are
from the automated baseline scan (`unwrap`, `println`, `let _`, `.ok()`,
inline `//`, `///`, files >300 lines, raw String IDs, raw `sqlx::query`,
`*Manager`, `#[allow(...)]`, `panic!`).

Per Wave 1 instructions, no crate is marked **CLEAN** in this baseline —
re-validation happens after the wave merges.

---

## Shared Layer

| Crate | Verdict | Total | Doc |
|-------|---------|-------|-----|
| systemprompt-models | CRITICAL | 43 | [systemprompt-models-2026-05.md](systemprompt-models-2026-05.md) |
| systemprompt-traits | CRITICAL | 17 | [systemprompt-traits-2026-05.md](systemprompt-traits-2026-05.md) |
| systemprompt-identifiers | NEEDS_WORK | 8 | [systemprompt-identifiers-2026-05.md](systemprompt-identifiers-2026-05.md) |
| systemprompt-extension | NEEDS_WORK | 3 | [systemprompt-extension-2026-05.md](systemprompt-extension-2026-05.md) |
| systemprompt-provider-contracts | NEEDS_WORK | 3 | [systemprompt-provider-contracts-2026-05.md](systemprompt-provider-contracts-2026-05.md) |
| systemprompt-client | NEEDS_WORK | 0 | [systemprompt-client-2026-05.md](systemprompt-client-2026-05.md) |
| systemprompt-template-provider | NEEDS_WORK | 0 | [systemprompt-template-provider-2026-05.md](systemprompt-template-provider-2026-05.md) |

## Infra Layer

| Crate | Verdict | Total | Doc |
|-------|---------|-------|-----|
| systemprompt-database | CRITICAL | 33 | [systemprompt-database-2026-05.md](systemprompt-database-2026-05.md) |
| systemprompt-events | NEEDS_WORK | 0 | [systemprompt-events-2026-05.md](systemprompt-events-2026-05.md) |
| systemprompt-security | NEEDS_WORK | 2 | [systemprompt-security-2026-05.md](systemprompt-security-2026-05.md) |
| systemprompt-config | CRITICAL | 20 | [systemprompt-config-2026-05.md](systemprompt-config-2026-05.md) |
| systemprompt-logging | CRITICAL | 58 | [systemprompt-logging-2026-05.md](systemprompt-logging-2026-05.md) |
| systemprompt-loader | NEEDS_WORK | 4 | [systemprompt-loader-2026-05.md](systemprompt-loader-2026-05.md) |
| systemprompt-cloud | NEEDS_WORK | 9 | [systemprompt-cloud-2026-05.md](systemprompt-cloud-2026-05.md) |

## Domain Layer

| Crate | Verdict | Total | Doc |
|-------|---------|-------|-----|
| systemprompt-users | CRITICAL | 16 | [systemprompt-users-2026-05.md](systemprompt-users-2026-05.md) |
| systemprompt-oauth | CRITICAL | 66 | [systemprompt-oauth-2026-05.md](systemprompt-oauth-2026-05.md) |
| systemprompt-files | NEEDS_WORK | 9 | [systemprompt-files-2026-05.md](systemprompt-files-2026-05.md) |
| systemprompt-analytics | CRITICAL | 46 | [systemprompt-analytics-2026-05.md](systemprompt-analytics-2026-05.md) |
| systemprompt-content | NEEDS_WORK | 9 | [systemprompt-content-2026-05.md](systemprompt-content-2026-05.md) |
| systemprompt-ai | CRITICAL | 17 | [systemprompt-ai-2026-05.md](systemprompt-ai-2026-05.md) |
| systemprompt-mcp | CRITICAL | 55 | [systemprompt-mcp-2026-05.md](systemprompt-mcp-2026-05.md) |
| systemprompt-agent | CLEAN | 0 | [agent-2026-04.md](agent-2026-04.md) |
| systemprompt-templates | NEEDS_WORK | 1 | [systemprompt-templates-2026-05.md](systemprompt-templates-2026-05.md) |

## App Layer

| Crate | Verdict | Total | Doc |
|-------|---------|-------|-----|
| systemprompt-runtime | NEEDS_WORK | 6 | [systemprompt-runtime-2026-05.md](systemprompt-runtime-2026-05.md) |
| systemprompt-scheduler | CRITICAL | 12 | [systemprompt-scheduler-2026-05.md](systemprompt-scheduler-2026-05.md) |
| systemprompt-generator | NEEDS_WORK | 2 | [systemprompt-generator-2026-05.md](systemprompt-generator-2026-05.md) |
| systemprompt-sync | NEEDS_WORK | 5 | [systemprompt-sync-2026-05.md](systemprompt-sync-2026-05.md) |

## Entry Layer

| Crate | Verdict | Total | Doc |
|-------|---------|-------|-----|
| systemprompt-api | CRITICAL | 109 | [systemprompt-api-2026-05.md](systemprompt-api-2026-05.md) |
| systemprompt-cli | CRITICAL | 99 | [systemprompt-cli-2026-05.md](systemprompt-cli-2026-05.md) |

## Facade

| Crate | Verdict | Total | Doc |
|-------|---------|-------|-----|
| systemprompt | NEEDS_WORK | 0 | [systemprompt-2026-05.md](systemprompt-2026-05.md) |

---

## Verdict Summary

| Verdict | Count |
|---------|-------|
| CLEAN | 1 (agent — pre-existing) |
| NEEDS_WORK | 16 |
| CRITICAL | 13 |

## Top 5 Worst Offenders (by total scored violations)

1. **systemprompt-api** — 109 (62 inline `//`, 27 `#[allow]`, 10 raw String IDs, 5 `let _ =`, 5 `.ok()` discards)
2. **systemprompt-cli** — 99 (73 inline `//`, 17 raw String IDs, 5 `let _ =`, 3 `*Manager`)
3. **systemprompt-oauth** — 66 (47 `#[allow]`, 15 inline `//`, 2 `let _ =`, 1 raw String ID)
4. **systemprompt-logging** — 58 (30 `#[allow]`, 23 `let _ =`, 3 inline `//`, 2 raw String IDs)
5. **systemprompt-mcp** — 55 (27 inline `//`, 18 `#[allow]`, 4 `.ok()`, 4 raw String IDs)

## Notes

- The `Total` column is the sum of scored buckets only; secondary signals
  (`anyhow::` and `async_trait` references) are recorded in each crate's
  doc but excluded from the score.
- `instructions/` is gitignored — the audit docs are committed via
  `git add -f`.
- Other Wave 1 agents are concurrently fixing source code. Re-run the
  scanner after the wave merges to promote crates to CLEAN.
