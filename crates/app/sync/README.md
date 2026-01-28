<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://systemprompt.io/documentation">Documentation</a></p>
</div>

---


# systemprompt-sync

Synchronization services for systemprompt.io - file, database, and crate deployment synchronization between local and cloud environments.

## Overview

This crate provides bidirectional sync capabilities for:

- **File Sync** - Upload/download service configuration files (agents, skills, content, config)
- **Database Sync** - Export/import users, skills, and contexts between local and cloud databases
- **Local Sync** - Synchronize content and skills between disk and local database
- **Crate Deploy** - Build and deploy Docker images to Fly.io

## File Structure

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

## Module Details

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

## Sync Directions

| Direction | Description |
|-----------|-------------|
| `Push` | Local to cloud (upload files, push database) |
| `Pull` | Cloud to local (download files, pull database) |
| `ToDisk` | Database to local files |
| `ToDatabase` | Local files to database |

## Usage

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

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-sync = "0.0.1"
```

## License

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt/blob/main/LICENSE) for details.
