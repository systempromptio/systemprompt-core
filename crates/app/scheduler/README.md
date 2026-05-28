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

Background jobs, cron tasks, and job-extension dispatch for systemprompt.io AI governance infrastructure. Tokio-backed scheduling discovers jobs via the `inventory` crate and executes them on configurable cron schedules.

**Layer**: App — orchestrates domain modules. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Part of the App layer in the systemprompt.io architecture.
**Infrastructure** · [Self-Hosted Deployment](https://systemprompt.io/features/self-hosted-ai-platform)

Background job scheduling and execution module. Discovers jobs via the `inventory` crate and executes them on configurable cron schedules.

## Architecture

```
src/
├── lib.rs                                    # Public exports and crate docs
├── error.rs                                  # SchedulerError, SchedulerResult
├── extension.rs                              # SchedulerExtension (schemas, jobs)
├── jobs/
│   ├── mod.rs                                # Job exports
│   ├── behavioral_analysis.rs                # Analyses fingerprint behaviour patterns
│   ├── cleanup_empty_contexts.rs             # Removes empty conversation contexts
│   ├── cleanup_inactive_sessions.rs          # Closes inactive sessions
│   ├── database_cleanup.rs                   # Orphaned logs, MCP, OAuth cleanup
│   ├── ghost_session_cleanup.rs              # Reaps abandoned ghost sessions
│   ├── malicious_ip_blacklist.rs             # Detects and blacklists malicious IPs
│   └── no_js_cleanup.rs                      # Cleans no-JavaScript fingerprint rows
├── models/
│   └── mod.rs                                # JobConfig, JobStatus, ScheduledJob, SchedulerConfig
├── repository/
│   ├── mod.rs                                # SchedulerRepository facade
│   ├── analytics/
│   │   └── mod.rs                            # Analytics cleanup queries
│   ├── jobs/
│   │   └── mod.rs                            # Scheduled job CRUD operations
│   └── security/
│       └── mod.rs                            # IP session queries for malicious detection
└── services/
    ├── mod.rs                                # Service exports
    ├── providers.rs                          # Provider-contract registration glue
    ├── service_management.rs                 # ServiceManagementService
    ├── scheduling/
    │   ├── mod.rs                            # SchedulerService - job discovery and execution
    │   └── dispatch.rs                       # Job dispatch and panic isolation
    └── orchestration/
        ├── mod.rs                            # Orchestration exports
        ├── process_cleanup/
        │   ├── mod.rs                        # Cross-platform process cleanup facade
        │   ├── posix.rs                      # POSIX signal + /proc-based cleanup
        │   └── winnt.rs                      # Windows process cleanup
        ├── reconciler.rs                     # Service state reconciliation
        ├── state_manager.rs                  # Service state verification
        ├── state_types.rs                    # DesiredStatus, RuntimeStatus, ServiceAction
        └── verified_state.rs                 # VerifiedServiceState with builder pattern
```

### jobs/

Background jobs that implement the `Job` trait from `systemprompt-traits`. Each job is registered via `inventory::submit!` for automatic discovery.

| Job | Schedule | Description |
|-----|----------|-------------|
| `CleanupInactiveSessionsJob` | Every 10 min | Closes sessions inactive for 1 hour |
| `CleanupEmptyContextsJob` | Every 2 hours | Removes conversation contexts with no messages |
| `DatabaseCleanupJob` | Daily at 3 AM | Deletes orphaned logs, MCP executions, expired OAuth tokens |
| `BehavioralAnalysisJob` | Hourly | Analyses fingerprint patterns, flags suspicious activity, bans repeat offenders |
| `MaliciousIpBlacklistJob` | Every 6 hours | Detects high-volume, scanner, datacenter, and high-risk country IPs |
| `GhostSessionCleanupJob` | Periodic | Reaps abandoned ghost sessions left by disconnected clients |
| `NoJsCleanupJob` | Periodic | Prunes no-JavaScript fingerprint rows past retention |

### models/

Domain types for the scheduler:

- **JobStatus** - Enum: `Success`, `Failed`, `Running`
- **JobConfig** - Per-job configuration
- **SchedulerConfig** - Scheduler-wide configuration
- **ScheduledJob** - Database model for job tracking

`SchedulerError` and `SchedulerResult` live in `error.rs` at the crate root.

### repository/

Data access layer for scheduler operations:

- **SchedulerRepository** - Facade combining job and analytics repositories
- **JobRepository** - CRUD for `scheduled_jobs` table
- **AnalyticsRepository** - Cleanup queries for `user_contexts`
- **SecurityRepository** - IP session queries for malicious IP detection

### services/

#### scheduling/

**SchedulerService** - Core scheduler that:
- Discovers jobs via the `inventory` crate (`submit_job!`)
- Registers jobs with `tokio-cron-scheduler`
- Dispatches jobs through `scheduling/dispatch.rs` with panic isolation
- Tracks job execution status
- Uses `SystemSpan` for structured logging

#### service_management.rs

**ServiceManagementService** - Service lifecycle operations:
- Query services by type
- Stop services (graceful or forced)
- Cleanup orphaned services

#### orchestration/

State machine for service reconciliation:

- **ServiceStateManager** - Verifies actual runtime state vs database state
- **ServiceReconciler** - Executes actions to reconcile desired vs actual state
- **VerifiedServiceState** - Immutable state snapshot with builder pattern
- **ProcessCleanup** - Low-level process management (kill, check port, etc.)

State types:
- **DesiredStatus** - `Enabled` | `Disabled`
- **RuntimeStatus** - `Running` | `Starting` | `Stopped` | `Crashed` | `Orphaned`
- **ServiceAction** - `None` | `Start` | `Stop` | `Restart` | `CleanupDb` | `CleanupProcess`

## Usage

```toml
[dependencies]
systemprompt-scheduler = "0.12.2"
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

let config = SchedulerConfig::with_system_admin(&system_admin);
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
