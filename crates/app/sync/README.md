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

- **File Sync** - Upload/download service configuration files (agents, skills, content, config)
- **Database Sync** - Export/import users, skills, and contexts between local and cloud databases
- **Local Sync** - Synchronize content and skills between disk and local database
- **Crate Deploy** - Build and deploy Docker images to Fly.io

## Architecture

```
src/
├── lib.rs                    # Crate root, public exports, SyncService orchestrator
├── error.rs                  # SyncError enum with MissingConfig variant
├── api_client.rs             # HTTP client for cloud API communication
├── files.rs                  # FileSyncService - tarball creation/extraction
├── crate_deploy.rs           # CrateDeployService - Docker build and deploy
├── database/
│   ├── mod.rs                # DatabaseSyncService, export models, import logic
│   └── upsert.rs             # Database upsert functions for users, skills, contexts
├── diff/
│   ├── mod.rs                # Diff module exports, hash computation functions
│   ├── content.rs            # ContentDiffCalculator - compare disk vs DB content
│   └── skills.rs             # SkillsDiffCalculator - compare disk vs DB skills
├── export/
│   ├── mod.rs                # Export utilities, YAML escape function
│   ├── content.rs            # Content markdown generation and file export
│   └── skills.rs             # Skill markdown/config generation and file export
├── local/
│   ├── mod.rs                # Local sync module exports
│   ├── content_sync.rs       # ContentLocalSync - bidirectional content sync
│   └── skills_sync.rs        # SkillsLocalSync - bidirectional skills sync
└── models/
    ├── mod.rs                # Model exports
    └── local_sync.rs         # Sync direction, diff items, and result types
```

### Module Details

| Module | Purpose |
|--------|---------|
| `SyncService` | Top-level orchestrator for file and database sync operations |
| `SyncApiClient` | HTTP client with direct sync and cloud API endpoints |
| `FileSyncService` | Creates/extracts gzipped tarballs for file sync |
| `DatabaseSyncService` | Exports and imports users, skills, contexts via SQL |
| `CrateDeployService` | Builds release, Docker image, and deploys to Fly.io |
| `ContentDiffCalculator` | Computes hash-based diffs between disk and database content |
| `SkillsDiffCalculator` | Computes hash-based diffs between disk and database skills |
| `ContentLocalSync` | Syncs content to/from disk using ingestion services |
| `SkillsLocalSync` | Syncs skills to/from disk using ingestion services |

### Sync Directions

| Direction | Description |
|-----------|-------------|
| `Push` | Local to cloud (upload files, push database) |
| `Pull` | Cloud to local (download files, pull database) |
| `ToDisk` | Database to local files |
| `ToDatabase` | Local files to database |

## Usage

```toml
[dependencies]
systemprompt-sync = "0.2.1"
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

let service = SyncService::new(config);
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
| `systemprompt-agent` | Skill repository and ingestion |
| `systemprompt-identifiers` | Typed identifiers (SkillId, SourceId, etc.) |
| `systemprompt-logging` | Tracing integration |

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-sync)** · **[docs.rs](https://docs.rs/systemprompt-sync)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>App layer · Own how your organization uses AI.</sub>

</div>
