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

# systemprompt-sync

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/app-sync.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/app-sync.svg">
    <img alt="systemprompt-sync terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/app-sync.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-sync.svg?style=flat-square)](https://crates.io/crates/systemprompt-sync)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-sync?style=flat-square)](https://docs.rs/systemprompt-sync)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Cloud sync services for systemprompt.io AI governance infrastructure. Provides file, database, and crate deployment synchronization across governance tenants — bidirectional sync between local and cloud environments.

**Layer**: App — orchestrates domain modules. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Part of the App layer in the systemprompt.io architecture.
**Infrastructure** · [Self-Hosted Deployment](https://systemprompt.io/features/self-hosted-ai-platform)

This crate provides bidirectional sync capabilities for:

- **File Sync** - Upload/download service configuration files (agents, skills, content, config) as gzipped tarballs
- **Database Sync** - Export/import contexts between local and cloud Postgres databases
- **Local Sync** - Synchronise content and access-control rules between disk and the local database
- **Crate Deploy** - Build and deploy Docker images to Fly.io
- **Scheduled Jobs** - Background sync jobs registered with the systemprompt scheduler

## Architecture

```
src/
├── lib.rs                    # Crate root, SyncService, SyncConfig, SyncOpState, public exports
├── error.rs                  # SyncError enum and SyncResult alias
├── files.rs                  # FileSyncService - tarball creation, manifest, push/pull
├── file_bundler.rs           # Internal tarball assembly helper (private)
├── crate_deploy.rs           # CrateDeployService - Docker build and deploy
├── api_client/
│   ├── mod.rs                # SyncApiClient - direct-sync vs cloud-relay endpoint selection
│   ├── response.rs           # Typed JSON / binary response handling
│   └── retry.rs              # RetryConfig and exponential backoff
├── database/
│   ├── mod.rs                # DatabaseSyncService, ContextExport, DatabaseExport
│   └── upsert.rs             # Compile-time-checked context upserts
├── diff/
│   ├── mod.rs                # Diff module exports, compute_content_hash
│   └── content.rs            # ContentDiffCalculator - disk vs database content
├── export/
│   ├── mod.rs                # Export utilities, escape_yaml
│   └── content.rs            # Content markdown generation and file export
├── local/
│   ├── mod.rs                # Local sync drivers
│   ├── content_sync.rs       # ContentLocalSync - bidirectional content sync
│   └── access_control_sync.rs # AccessControlLocalSync - bidirectional ACL sync
├── jobs/
│   ├── mod.rs                # Scheduled jobs
│   ├── content_sync.rs       # ContentSyncJob
│   └── access_control_sync.rs # AccessControlSyncJob
└── models/
    ├── mod.rs                # Model exports
    └── local_sync.rs         # LocalSyncDirection, DiffStatus, diff items, result types
```

### Module Details

| Module | Purpose |
|--------|---------|
| `SyncService` | Top-level orchestrator for file and database sync operations |
| `SyncConfig` / `SyncConfigBuilder` | Configuration façade with builder semantics |
| `SyncApiClient` | HTTP client with direct-sync and cloud-relay endpoints |
| `FileSyncService` | Creates and extracts gzipped tarballs for file sync |
| `DatabaseSyncService` | Round-trips context records between local and cloud Postgres |
| `CrateDeployService` | Builds release artefacts, Docker image, and deploys to Fly.io |
| `ContentDiffCalculator` | Hash-based diff between disk and database content |
| `ContentLocalSync` | Syncs content to and from disk via ingestion services |
| `AccessControlLocalSync` | Syncs access-control rules to and from disk |
| `ContentSyncJob` / `AccessControlSyncJob` | Scheduler-registered background jobs |

### Sync Directions

| Direction | Description |
|-----------|-------------|
| `Push` | Local to cloud (upload files, push database) |
| `Pull` | Cloud to local (download files, pull database) |

Local disk-database sync uses `LocalSyncDirection` (`ToDisk`, `ToDatabase`) on `LocalSyncResult`.

### Operation State

`SyncService::sync_all` returns per-operation results with a `SyncOpState`:

| State | Meaning |
|-------|---------|
| `NotStarted` | Operation was skipped (e.g. missing local database URL) |
| `Partial { completed, total }` | Operation imported a subset of items before failing |
| `Completed` | Operation finished successfully |
| `Failed` | Operation failed without partial progress |

## Usage

```toml
[dependencies]
systemprompt-sync = "0.12.2"
```

```rust
use systemprompt_sync::{SyncConfig, SyncService, SyncDirection};

let config = SyncConfig::builder(
    "tenant-id",
    "https://api.systemprompt.io",
    "api-token",
    "./services",
)
.with_direction(SyncDirection::Push)
.with_dry_run(false)
.build();

let service = SyncService::new(config)?;
let results = service.sync_all().await?;
```

## Error Handling

The crate uses `SyncError` for all error conditions:

- `MissingConfig` - Required configuration (e.g., `DATABASE_URL`) not set
- `ApiError` - HTTP API failures with status code and message
- `Database` - SQL/connection errors
- `Io` - File system errors
- `Unauthorized` - Authentication required
- `CommandFailed` - Shell command execution failures

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-database` | Database pool and provider traits |
| `systemprompt-content` | Content repository and ingestion |
| `systemprompt-agent` | Agent and access-control repositories |
| `systemprompt-security` | Access-control rule models |
| `systemprompt-identifiers` | Typed identifiers (`TenantId`, `ContextId`, `UserId`, etc.) |
| `systemprompt-logging` | Tracing integration |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-sync)** · **[docs.rs](https://docs.rs/systemprompt-sync)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>App layer · Own how your organization uses AI.</sub>

</div>
