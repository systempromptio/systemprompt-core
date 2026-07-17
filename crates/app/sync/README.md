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
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

The bridge between the deployment you run locally and the one you run in the cloud, with your data moving on your terms in both directions. This crate pushes and pulls service files, database contexts, and access-control rules, and drives the full tenant deploy pipeline to Fly.io.

**Layer**: App, orchestrates domain modules. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

Bidirectional sync across five paths:

- **File sync** uploads and downloads service configuration files (agents, skills, content, config) as gzipped tarballs.
- **Database sync** exports and imports contexts between local and cloud Postgres.
- **Local sync** reconciles content and access-control rules between disk and the local database.
- **Deploy** sequences the full `cloud deploy` pipeline: pre-sync, build, Docker image, and Fly.io release.
- **Scheduled jobs** run content and access-control sync in the background via the systemprompt scheduler.

**Infrastructure** · [Self-Hosted Deployment](https://systemprompt.io/features/self-hosted-ai-platform)

## Modules

| Module | Purpose |
|--------|---------|
| `SyncService` | Top-level orchestrator for file and database sync (`lib.rs`) |
| `config` | `SyncConfig` / `SyncConfigBuilder`, configuration with builder semantics |
| `api_client` | `SyncApiClient`, direct-sync versus cloud-relay endpoint selection, typed responses, retry with backoff |
| `files` | `FileSyncService` tarball creation and push/pull; `file_bundler/` assembles the archive |
| `database` | `DatabaseSyncService`, round-trips context records between local and cloud Postgres |
| `deploy` | `DeployOrchestrator`, the full tenant deploy pipeline (pre-sync, build, image, release) |
| `crate_deploy` | `CrateDeployService`, release-artefact build and Docker deploy |
| `diff` | `ContentDiffCalculator`, hash-based diff between disk and database content |
| `local` | `ContentLocalSync` and `AccessControlLocalSync`, bidirectional disk/database sync |
| `jobs` | `ContentSyncJob` and `AccessControlSyncJob`, scheduler-registered background jobs |
| `models` | `LocalSyncDirection`, `DiffStatus`, diff items, and result types |
| `error` | `SyncError` and `SyncResult` |

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
systemprompt-sync = "0.21"
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

The crate uses `SyncError` for all error conditions. Representative variants:

- `MissingConfig` - Required configuration (e.g., `DATABASE_URL`) not set
- `ApiError { status, message }` - HTTP API failures with status code and message
- `Unauthorized` - Authentication required
- `CommandFailed { command }` - Shell command execution failure
- `PartialImport { completed, total, message }` - Import failed after partial progress
- `TarballUnsafe` - Rejected an unsafe tarball entry
- `Database` - SQL / connection errors
- `Cloud` / `ConfigLoad` / `ExtensionDiscovery` - Wrapped errors from the cloud, loader, and extension crates

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-cloud` | Cloud API client and tenant operations |
| `systemprompt-database` | Database pool and provider traits |
| `systemprompt-content` | Content repository and ingestion |
| `systemprompt-security` | Access-control rule models |
| `systemprompt-loader` | Config and extension discovery |
| `systemprompt-extension` | Extension discovery |
| `systemprompt-identifiers` | Typed identifiers (`TenantId`, `ContextId`, `UserId`, etc.) |
| `systemprompt-models` / `systemprompt-traits` / `systemprompt-provider-contracts` | Domain types, trait interfaces, and provider registration |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-sync)** · **[docs.rs](https://docs.rs/systemprompt-sync)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>App layer · Own how your organization uses AI.</sub>

</div>
