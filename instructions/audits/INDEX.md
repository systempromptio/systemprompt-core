# systemprompt-core Audit Index

**Generated:** 2026-05-04
**Wave:** Wave A complete (shared layer flipped CLEAN), Wave B complete
(infra + oauth flipped CLEAN). Wave C/D/E target the remaining CRITICAL
domain, app, and entry crates.

This index lists every published crate in dependency-layer order. Counts are
from the automated baseline scan (`unwrap`, `println`, `let _`, `.ok()`,
inline `//`, `///`, files >300 lines, raw String IDs, raw `sqlx::query`,
`*Manager`, `#[allow(...)]`, `panic!`).

Wave A merged at tag `compliance-wave-A` flipped 7 shared-layer crates
CLEAN. Wave B merged at tag `compliance-wave-B` flipped 5 more crates
CLEAN: `events`, `security`, `loader`, `database`, `logging`, `config`,
`cloud`, plus the pulled-forward `oauth` (originally a Wave C target).

---

## Shared Layer

| Crate | Verdict | Total | Doc |
|-------|---------|-------|-----|
| systemprompt-models | CLEAN | 0 | [systemprompt-models-2026-05.md](systemprompt-models-2026-05.md) |
| systemprompt-traits | CLEAN | 0 | [systemprompt-traits-2026-05.md](systemprompt-traits-2026-05.md) |
| systemprompt-identifiers | CLEAN | 0 | [systemprompt-identifiers-2026-05.md](systemprompt-identifiers-2026-05.md) |
| systemprompt-extension | CLEAN | 0 | [systemprompt-extension-2026-05.md](systemprompt-extension-2026-05.md) |
| systemprompt-provider-contracts | CLEAN | 0 | [systemprompt-provider-contracts-2026-05.md](systemprompt-provider-contracts-2026-05.md) |
| systemprompt-client | CLEAN | 0 | [systemprompt-client-2026-05.md](systemprompt-client-2026-05.md) |
| systemprompt-template-provider | CLEAN | 0 | [systemprompt-template-provider-2026-05.md](systemprompt-template-provider-2026-05.md) |

## Infra Layer

| Crate | Verdict | Total | Doc |
|-------|---------|-------|-----|
| systemprompt-database | CLEAN* | 0 | [systemprompt-database-2026-05.md](systemprompt-database-2026-05.md) |
| systemprompt-events | CLEAN | 0 | [systemprompt-events-2026-05.md](systemprompt-events-2026-05.md) |
| systemprompt-security | CLEAN | 0 | [systemprompt-security-2026-05.md](systemprompt-security-2026-05.md) |
| systemprompt-config | CLEAN | 0 | [systemprompt-config-2026-05.md](systemprompt-config-2026-05.md) |
| systemprompt-logging | CLEAN | 0 | [systemprompt-logging-2026-05.md](systemprompt-logging-2026-05.md) |
| systemprompt-loader | CLEAN | 0 | [systemprompt-loader-2026-05.md](systemprompt-loader-2026-05.md) |
| systemprompt-cloud | CLEAN | 0 | [systemprompt-cloud-2026-05.md](systemprompt-cloud-2026-05.md) |

\* `database` is CLEAN-with-residual: `DatabaseProvider` /
`DatabaseTransaction` traits keep `anyhow::Result` because they are
`dyn`-used across crates; full typed-error cutover deferred to a future
wave that touches all consumers in lockstep.

## Domain Layer

| Crate | Verdict | Total | Doc |
|-------|---------|-------|-----|
| systemprompt-users | CRITICAL | 16 | [systemprompt-users-2026-05.md](systemprompt-users-2026-05.md) |
| systemprompt-oauth | CLEAN | 0 | [systemprompt-oauth-2026-05.md](systemprompt-oauth-2026-05.md) |
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

Entry binaries (`api`, `cli`) are **exempt** from §3a Public-API Hygiene rules
that target published library crates: they may keep `anyhow::Error` at the
HTTP / process boundary, and they are not required to carry `///` rustdoc on
internal items. They are still subject to: file-size (`just file-size`),
banned-pattern (`just check-bans`), `let _` / `.ok()` carve-out comments,
sqlx allowlist, lint floor (`-D warnings`), and `cargo deny` / `cargo audit`.
Internal command modules SHOULD still adopt typed errors where it doesn't
require pushing `anyhow` back into a library crate's public API.

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
| CLEAN | 13 (1 pre-existing agent + 7 Wave A shared + 5 Wave B infra + oauth pulled forward) |
| NEEDS_WORK | 9 |
| CRITICAL | 8 |

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
- Wave A and Wave B have flipped 12 crates to CLEAN. Wave C/D/E remain to
  flip the other domain (users, files, analytics, ai, mcp, content,
  templates), app (runtime, scheduler, generator, sync), and entry
  (api, cli) crates plus the facade.
