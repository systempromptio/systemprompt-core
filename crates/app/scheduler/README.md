<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-scheduler

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/app-scheduler.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/app-scheduler.svg">
    <img alt="systemprompt-scheduler terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/app-scheduler.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-scheduler.svg?style=flat-square)](https://crates.io/crates/systemprompt-scheduler)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-scheduler?style=flat-square)](https://docs.rs/systemprompt-scheduler)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

The maintenance and enforcement work that keeps a self-hosted deployment healthy runs here, inside your own binary. Session cleanup, orphaned-record reaping, and malicious-IP blacklisting run as background jobs on cron schedules, discovered at compile time through the `inventory` crate and dispatched with panic isolation on a Tokio runtime.

**Layer**: App, orchestrates domain modules. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Jobs are discovered through `inventory` and executed on configurable cron schedules. Alongside job execution, the crate reconciles long-running service state: it compares the desired configuration against the observed runtime and takes the action that closes the gap.

**Infrastructure** · [Self-Hosted Deployment](https://systemprompt.io/features/self-hosted-ai-platform)

## Modules

| Module | Purpose |
|--------|---------|
| `jobs` | Background jobs implementing the `Job` trait, each registered via `inventory` |
| `services/scheduling` | `SchedulerService` job discovery and execution, plus `dispatch`, `bootstrap`, distributed `lock`, and job `owners` resolution |
| `services/job_execution` | Single-job execution path with status tracking |
| `services/service_management` | `ServiceManagementService` lifecycle operations (query, stop, cleanup) |
| `services/orchestration` | Service state reconciliation (`ServiceStateVerifier`, `ServiceReconciler`, `VerifiedServiceState`, cross-platform `process_cleanup`) |
| `services/plans` | Declarative service plans consumed by the reconciler |
| `models` | `JobConfig`, `JobStatus`, `ScheduledJob`, `SchedulerConfig` |
| `repository` | `SchedulerRepository` facade over job, analytics, and security queries |
| `error` | `SchedulerError`, `SchedulerResult` |
| `extension` | `SchedulerExtension` (schemas, registered jobs) |

### Jobs

Each job implements the `Job` trait from `systemprompt-traits` and is registered via `inventory` for automatic discovery.

| Job | Schedule | Description |
|-----|----------|-------------|
| `CleanupInactiveSessionsJob` | Every 10 min | Closes sessions inactive for 1 hour |
| `CleanupEmptyContextsJob` | Every 2 hours | Removes conversation contexts with no messages |
| `DatabaseCleanupJob` | Daily at 3 AM | Deletes orphaned logs, MCP executions, expired OAuth tokens |
| `BehavioralAnalysisJob` | Hourly | Analyses fingerprint patterns, flags suspicious activity, bans repeat offenders |
| `MaliciousIpBlacklistJob` | Every 6 hours | Detects high-volume, scanner, datacenter, and high-risk country IPs |
| `GhostSessionCleanupJob` | Periodic | Reaps abandoned ghost sessions left by disconnected clients |
| `NoJsCleanupJob` | Periodic | Prunes no-JavaScript fingerprint rows past retention |

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
systemprompt-scheduler = "0.21"
```

### Job Discovery

Jobs are discovered via the `inventory` crate. Any crate can register jobs:

```rust
use systemprompt_provider_contracts::submit_job;

submit_job!(MyCustomJob);
```

The scheduler discovers all registered jobs at startup and schedules them based on configuration.

```rust
use systemprompt_scheduler::{SchedulerService, SchedulerConfig};

let config = SchedulerConfig::with_system_admin();
let service = SchedulerService::new(config, db_pool, app_context)?;
service.start().await?;
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-runtime` | AppContext |
| `systemprompt-database` | Database pool and repositories |
| `systemprompt-extension` | Extension trait |
| `systemprompt-logging` | SystemSpan for tracing |
| `systemprompt-analytics` | SessionRepository, FingerprintRepository |
| `systemprompt-users` | BannedIpRepository |
| `systemprompt-traits` | Job trait definition |
| `systemprompt-provider-contracts` | `submit_job!` macro and provider registry |
| `systemprompt-identifiers` | ScheduledJobId |
| `systemprompt-models` | Config types |
| `tokio-cron-scheduler` | Cron scheduling runtime |
| `nix` (unix) | POSIX process signals for orchestration cleanup |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-scheduler)** · **[docs.rs](https://docs.rs/systemprompt-scheduler)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>App layer · Own how your organization uses AI.</sub>

</div>
