<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://docs.systemprompt.io">Documentation</a></p>
</div>

---


# systemprompt-scheduler

Core scheduler module for systemprompt.io OS - background jobs and cron tasks.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-scheduler.svg)](https://crates.io/crates/systemprompt-scheduler)
[![Documentation](https://docs.rs/systemprompt-scheduler/badge.svg)](https://docs.rs/systemprompt-scheduler)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](https://github.com/systempromptio/systemprompt/blob/main/LICENSE)

## Overview

**Part of the App layer in the systemprompt.io architecture.**

Background job scheduling and execution module. Discovers jobs via the `inventory` crate and executes them on configurable cron schedules.

## File Structure

```
src/
├── lib.rs                                    # Public exports
├── jobs/
│   ├── mod.rs                                # Job exports
│   ├── behavioral_analysis.rs                # Analyzes fingerprint behavior patterns
│   ├── cleanup_empty_contexts.rs             # Removes empty conversation contexts
│   ├── cleanup_inactive_sessions.rs          # Closes inactive sessions
│   ├── database_cleanup.rs                   # Orphaned logs, MCP, OAuth cleanup
│   └── malicious_ip_blacklist.rs             # Detects and blacklists malicious IPs
├── models/
│   └── mod.rs                                # JobStatus, SchedulerError, ScheduledJob
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
    ├── service_management.rs                 # Service lifecycle management
    ├── scheduling/
    │   └── mod.rs                            # SchedulerService - job discovery and execution
    └── orchestration/
        ├── mod.rs                            # Orchestration exports
        ├── process_cleanup.rs                # Process management utilities
        ├── reconciler.rs                     # Service state reconciliation
        ├── state_manager.rs                  # Service state verification
        ├── state_types.rs                    # DesiredStatus, RuntimeStatus, ServiceAction
        └── verified_state.rs                 # VerifiedServiceState with builder pattern
```

## Modules

### jobs/

Background jobs that implement the `Job` trait from `systemprompt-traits`. Each job is registered via `inventory::submit!` for automatic discovery.

| Job | Schedule | Description |
|-----|----------|-------------|
| `CleanupInactiveSessionsJob` | Every 10 min | Closes sessions inactive for 1 hour |
| `CleanupEmptyContextsJob` | Every 2 hours | Removes conversation contexts with no messages |
| `DatabaseCleanupJob` | Daily at 3 AM | Deletes orphaned logs, MCP executions, expired OAuth tokens |
| `BehavioralAnalysisJob` | Hourly | Analyzes fingerprint patterns, flags suspicious activity, bans repeat offenders |
| `MaliciousIpBlacklistJob` | Every 6 hours | Detects high-volume, scanner, datacenter, and high-risk country IPs |

### models/

Domain types for the scheduler:

- **JobStatus** - Enum: `Success`, `Failed`, `Running`
- **SchedulerError** - Error types with `thiserror` derive
- **ScheduledJob** - Database model for job tracking

### repository/

Data access layer for scheduler operations:

- **SchedulerRepository** - Facade combining job and analytics repositories
- **JobRepository** - CRUD for `scheduled_jobs` table
- **AnalyticsRepository** - Cleanup queries for `user_contexts`
- **SecurityRepository** - IP session queries for malicious IP detection

### services/

#### scheduling/

**SchedulerService** - Core scheduler that:
- Discovers jobs via `inventory` crate
- Registers jobs with `tokio-cron-scheduler`
- Tracks job execution status
- Uses `SystemSpan` for structured logging

#### service_management/

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

## Job Discovery

Jobs are discovered via the `inventory` crate. Any crate can register jobs:

```rust
inventory::submit! { &MyCustomJob }
```

The scheduler discovers all registered jobs at startup and schedules them based on configuration.

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-runtime` | AppContext |
| `systemprompt-database` | Database pool and repositories |
| `systemprompt-logging` | SystemSpan for tracing |
| `systemprompt-analytics` | SessionRepository, FingerprintRepository |
| `systemprompt-users` | BannedIpRepository |
| `systemprompt-traits` | Job trait definition |
| `systemprompt-identifiers` | ScheduledJobId |
| `systemprompt-models` | Config types |

## Usage

```rust
use systemprompt_scheduler::{SchedulerService, SchedulerConfig};

let config = SchedulerConfig::from_context(&app_context);
let service = SchedulerService::new(config, db_pool, app_context)?;
service.start().await?;
```

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-scheduler = "0.0.1"
```

## License

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt/blob/main/LICENSE) for details.
