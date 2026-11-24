# systemprompt-scheduler

[![Crates.io](https://img.shields.io/crates/v/systemprompt-scheduler.svg?style=flat-square)](https://crates.io/crates/systemprompt-scheduler)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-scheduler?style=flat-square)](https://docs.rs/systemprompt-scheduler)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Schedules and dispatches background jobs, including session cleanup, service reconciliation and maintenance, using compile-time job registration and Tokio execution.

**Layer**: App, orchestrates domain modules. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Jobs are the ones the admitted composition's members contribute, executed on configurable cron schedules. Alongside job execution, the crate reconciles long-running service state: it compares the desired configuration against the observed runtime and takes the action that closes the gap.

**Infrastructure** · [Self-Hosted Deployment](https://systemprompt.io/features/self-hosted-ai-platform)

## Modules

| Module | Purpose |
|--------|---------|
| `jobs` | Background jobs implementing the `Job` trait, contributed by the scheduler member |
| `services/scheduling` | `SchedulerService` job discovery and execution, plus `dispatch`, `bootstrap`, distributed `lock`, and job `owners` resolution |
| `services/job_execution` | Single-job execution path with status tracking |
| `services/service_management` | `ServiceManagementService` lifecycle operations (query, stop, cleanup) |
| `services/orchestration` | Service state reconciliation (`ServiceStateVerifier`, `ServiceReconciler`, `VerifiedServiceState`, cross-platform `process_cleanup`) |
| `services/plans` | Declarative service plans consumed by the reconciler |
| `models` | `JobConfig`, `JobStatus`, `ScheduledJob`, `SchedulerConfig` |
| `repository` | `SchedulerRepository` facade over job and lock queries; config-plane, licence and OTLP state |
| `error` | `SchedulerError`, `SchedulerResult` |
| `extension` | `SchedulerExtension` (schemas, migrations, kernel jobs) |

### Jobs

Each job implements the `Job` trait from `systemprompt-provider-contracts` and is contributed by its member with `Contributions::job`.

The scheduler hosts kernel jobs only. Capability members contribute their own jobs
(for example `behavioral_analysis` and `malicious_ip_blacklist` in
`systemprompt-analytics`, `usage_anomaly_scan` in `systemprompt-ai`,
`cleanup_empty_contexts` in `systemprompt-agent`), and contribute to two kernel
jobs that iterate contributions: `database_cleanup` deletes through the
retention each table's owner declares (`Contributions::retention`), and
`backfill_rollups` drains the projections owners declare with
`Contributions::projection`.

| Job | Schedule | Description |
|-----|----------|-------------|
| `DatabaseCleanupJob` | Daily at 3 AM | Prunes every admitted retention target past its profile window, orphaned logs and expired archived users |
| `BackfillRollupsJob` | Every minute | Drains the hourly usage rollups owners declare |

### Service reconciliation

`ServiceStateVerifier` (in `services/orchestration/state_verifier.rs`) compares observed runtime state against the database record; `ServiceReconciler` executes the action that reconciles desired versus actual state.

| Type | Values |
|------|--------|
| `DesiredStatus` | `Enabled`, `Disabled` |
| `RuntimeStatus` | `Running`, `Starting`, `Stopped`, `Crashed`, `Orphaned` |
| `ServiceAction` | `None`, `Start`, `Stop`, `Restart`, `CleanupDb`, `CleanupProcess` |

## Usage

```toml
[dependencies]
systemprompt-scheduler = "0.86"
```

### Job Discovery

A member contributes a job from its `contribute`; the kernel builds one
`AdmittedSchedule` from the admitted composition, stamping each job with
its owner:

```rust
fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
    c.job(MyCustomJob);
    Ok(())
}
```

The scheduler schedules the jobs of the admitted members; a member the
profile disables is not admitted, so its jobs are not scheduled. Start never
deletes a `scheduled_jobs` row: nodes of different releases or compositions
that share a database keep each other's rows, `enabled` flags and run
history, and a node never claims or runs a job its composition does not
admit. A tick skips a job whose row an operator disabled (`infra jobs
disable`). Rows no job of the admitted composition names are removed by
`infra jobs prune`, which includes the rows of members the profile disables
and is run with a binary and profile that admit every capability the cluster
runs. A manual `infra jobs run` runs a job whatever its `enabled` flag.

A `cluster` job takes a Postgres advisory lock for its run, and a replica that
reaches the same tick within 900 ms of the recorded run skips it. The lock is
released when the run ends, so delivery is at-least-once: a replica whose tick
arrives after that window runs the job again, and a tick that falls while no
scheduling node is up is not caught up. Jobs must be idempotent. Every job run executes
inside the job-run scopes admitted members contribute
(`Contributions::job_run_scope`; the files member's sets the system document
actor).

```rust
use systemprompt_scheduler::{SchedulerService, SchedulerConfig};

let config = SchedulerConfig::with_system_admin();
let service = SchedulerService::new(config, db_pool, app_context, schedule);
let startup = service.start().await?;
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-runtime` | AppContext |
| `systemprompt-database` | Database pool and repositories |
| `systemprompt-extension` | Extension trait, `Contributions`, `AdmittedSchedule` |
| `systemprompt-logging` | SystemSpan for tracing |
| `systemprompt-users` | BannedIpRepository |
| `systemprompt-traits` | `JobRunScope`, retention traits |
| `systemprompt-provider-contracts` | `Job` trait and provider contracts |
| `systemprompt-identifiers` | ScheduledJobId |
| `systemprompt-models` | Config types |
| `tokio-cron-scheduler` | Cron scheduling runtime |
| `nix` (unix) | POSIX process signals for orchestration cleanup |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---
