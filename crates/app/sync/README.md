# systemprompt-sync

Synchronization module for content and skills between local disk and database.

## Structure

```
src/
├── lib.rs                    # Public exports
├── api_client.rs             # API client
├── crate_deploy.rs           # Crate deployment
├── database.rs               # Database sync
├── error.rs                  # Error types
├── files.rs                  # File operations
├── diff/
│   ├── mod.rs                # Diff calculation exports
│   ├── content.rs            # Content diff calculator
│   └── skills.rs             # Skills diff calculator
├── export/
│   ├── mod.rs                # Export utilities
│   ├── content.rs            # Content export to disk
│   └── skills.rs             # Skills export to disk
├── local/
│   ├── mod.rs                # Local sync exports
│   ├── content_sync.rs       # Content sync service
│   └── skills_sync.rs        # Skills sync service
└── models/
    ├── mod.rs                # Model exports
    └── local_sync.rs         # Sync models and types
```

## Sync Operations

| Direction | Description |
|-----------|-------------|
| ToDisk | Export from database to local files |
| ToDatabase | Import from local files to database |

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-core-database` | Database access |
| `systemprompt-core-content` | Content repository |
| `systemprompt-core-agent` | Skill repository |
| `systemprompt-identifiers` | Typed identifiers |
