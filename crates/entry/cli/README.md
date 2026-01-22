# systemprompt.io CLI

**Layer:** Entry
**Binary:** `systemprompt`

Command-line interface for systemprompt.io OS. Every command supports both human-friendly interactive mode and agent-friendly non-interactive mode.

---

## Overview

The CLI provides comprehensive management of systemprompt.io OS including:

- **Agent Management** - Create, configure, and orchestrate AI agents
- **Cloud Operations** - Deploy, sync, and manage cloud tenants
- **Content Management** - Ingest, publish, and manage content
- **Infrastructure** - Database, services, jobs, and logging
- **Analytics** - Usage metrics, costs, and performance insights
- **Plugin System** - MCP servers and capability extensions

---

## File Structure

```
src/
├── lib.rs                      # CLI entrypoint, command routing, initialization
├── bootstrap.rs                # Bootstrap sequence for profile/secrets/paths
├── cli_settings.rs             # CliConfig, OutputFormat, VerbosityLevel
├── requirements.rs             # Command requirements specification (HasRequirements trait)
├── session.rs                  # Session lifecycle management (JWT, context)
│
├── commands/
│   ├── mod.rs                  # Root command enum and dispatch
│   │
│   ├── admin/                  # systemprompt admin [...]
│   │   ├── mod.rs
│   │   ├── agents/             # Agent CRUD and orchestration
│   │   │   ├── mod.rs
│   │   │   ├── create.rs       # Create new agent
│   │   │   ├── delete.rs       # Delete agent(s)
│   │   │   ├── edit.rs         # Edit agent config
│   │   │   ├── list.rs         # List all agents
│   │   │   ├── logs.rs         # View agent logs
│   │   │   ├── message.rs      # Send message to agent
│   │   │   ├── registry.rs     # Agent registry operations
│   │   │   ├── run.rs          # Run agent
│   │   │   ├── shared.rs       # Shared utilities
│   │   │   ├── show.rs         # Show agent details
│   │   │   ├── status.rs       # Agent status
│   │   │   ├── task.rs         # Task operations
│   │   │   ├── tools.rs        # Agent tool management
│   │   │   ├── types.rs        # Output types
│   │   │   └── validate.rs     # Validate agent config
│   │   │
│   │   ├── config/             # Configuration management
│   │   │   ├── mod.rs
│   │   │   ├── edit.rs         # Edit config
│   │   │   ├── get.rs          # Get config value
│   │   │   ├── list.rs         # List config keys
│   │   │   ├── paths.rs        # Path configuration
│   │   │   ├── provider.rs     # Provider config
│   │   │   ├── rate_limits/    # Rate limit config (submodule)
│   │   │   ├── rate_limits.rs  # Rate limit management
│   │   │   ├── runtime.rs      # Runtime config
│   │   │   ├── security.rs     # Security config
│   │   │   ├── server.rs       # Server config
│   │   │   ├── set.rs          # Set config value
│   │   │   ├── show.rs         # Show config
│   │   │   ├── types.rs        # Config output types
│   │   │   └── validate.rs     # Validate config
│   │   │
│   │   ├── session/            # CLI session management
│   │   │   ├── mod.rs
│   │   │   ├── list.rs         # List sessions/profiles
│   │   │   ├── show.rs         # Show session details
│   │   │   └── switch.rs       # Switch session
│   │   │
│   │   ├── setup/              # Initial setup wizard
│   │   │   ├── mod.rs
│   │   │   ├── docker.rs       # Docker setup
│   │   │   ├── postgres.rs     # PostgreSQL setup
│   │   │   ├── profile.rs      # Profile setup
│   │   │   ├── secrets.rs      # Secrets setup
│   │   │   ├── types.rs        # Setup output types
│   │   │   └── wizard.rs       # Interactive wizard
│   │   │
│   │   └── users/              # User administration
│   │       ├── mod.rs
│   │       ├── ban/            # Ban management
│   │       │   ├── mod.rs
│   │       │   ├── add.rs
│   │       │   ├── check.rs
│   │       │   ├── cleanup.rs
│   │       │   ├── list.rs
│   │       │   └── remove.rs
│   │       ├── bulk/           # Bulk operations
│   │       │   ├── mod.rs
│   │       │   ├── delete.rs
│   │       │   └── update.rs
│   │       ├── role/           # Role management
│   │       │   ├── mod.rs
│   │       │   ├── assign.rs
│   │       │   ├── demote.rs
│   │       │   └── promote.rs
│   │       ├── session/        # User session management
│   │       │   ├── mod.rs
│   │       │   ├── cleanup.rs
│   │       │   ├── end.rs
│   │       │   └── list.rs
│   │       ├── count.rs
│   │       ├── create.rs
│   │       ├── delete.rs
│   │       ├── export.rs
│   │       ├── list.rs
│   │       ├── merge.rs
│   │       ├── search.rs
│   │       ├── show.rs
│   │       ├── stats.rs
│   │       ├── types.rs
│   │       └── update.rs
│   │
│   ├── analytics/              # systemprompt analytics [...]
│   │   ├── mod.rs
│   │   ├── overview.rs         # Analytics dashboard
│   │   ├── agents/             # Agent analytics
│   │   │   ├── mod.rs
│   │   │   ├── list.rs
│   │   │   ├── show.rs
│   │   │   ├── stats.rs
│   │   │   └── trends.rs
│   │   ├── content/            # Content analytics
│   │   │   ├── mod.rs
│   │   │   ├── stats.rs
│   │   │   ├── top.rs
│   │   │   └── trends.rs
│   │   ├── conversations/      # Conversation analytics
│   │   │   ├── mod.rs
│   │   │   ├── list.rs
│   │   │   ├── stats.rs
│   │   │   └── trends.rs
│   │   ├── costs/              # Cost analytics
│   │   │   ├── mod.rs
│   │   │   ├── breakdown.rs
│   │   │   ├── summary.rs
│   │   │   └── trends.rs
│   │   ├── requests/           # Request analytics
│   │   │   ├── mod.rs
│   │   │   ├── list.rs
│   │   │   ├── models.rs
│   │   │   ├── stats.rs
│   │   │   └── trends.rs
│   │   ├── sessions/           # Session analytics
│   │   │   ├── mod.rs
│   │   │   ├── live.rs
│   │   │   ├── stats.rs
│   │   │   └── trends.rs
│   │   ├── shared/             # Shared analytics utilities
│   │   │   ├── mod.rs
│   │   │   ├── export.rs
│   │   │   ├── output.rs
│   │   │   └── time.rs
│   │   ├── tools/              # Tool usage analytics
│   │   │   ├── mod.rs
│   │   │   ├── list.rs
│   │   │   ├── show.rs
│   │   │   ├── stats.rs
│   │   │   └── trends.rs
│   │   └── traffic/            # Traffic analytics
│   │       ├── mod.rs
│   │       ├── bots.rs
│   │       ├── devices.rs
│   │       ├── geo.rs
│   │       └── sources.rs
│   │
│   ├── build/                  # systemprompt build [...]
│   │   ├── mod.rs
│   │   ├── core.rs             # Build core
│   │   ├── mcp.rs              # Build MCP servers
│   │   ├── types.rs            # Build output types
│   │   └── web.rs              # Build web assets
│   │
│   ├── cloud/                  # systemprompt cloud [...]
│   │   ├── mod.rs
│   │   ├── db.rs               # Cloud database operations
│   │   ├── dockerfile.rs       # Dockerfile generation
│   │   ├── domain.rs           # Domain management
│   │   ├── restart.rs          # Restart cloud services
│   │   ├── secrets.rs          # Cloud secrets management
│   │   ├── status.rs           # Cloud status
│   │   ├── auth/               # Cloud authentication
│   │   │   ├── mod.rs
│   │   │   ├── login.rs
│   │   │   ├── logout.rs
│   │   │   └── whoami.rs
│   │   ├── deploy/             # Cloud deployment
│   │   │   ├── mod.rs
│   │   │   └── select.rs
│   │   ├── init/               # Cloud project init
│   │   │   ├── mod.rs
│   │   │   └── templates.rs
│   │   ├── profile/            # Profile management
│   │   │   ├── mod.rs
│   │   │   ├── api_keys.rs
│   │   │   ├── builders.rs
│   │   │   ├── create.rs
│   │   │   ├── create_setup.rs
│   │   │   ├── create_tenant.rs
│   │   │   ├── delete.rs
│   │   │   ├── edit.rs
│   │   │   ├── edit_secrets.rs
│   │   │   ├── edit_settings.rs
│   │   │   ├── list.rs
│   │   │   ├── show.rs
│   │   │   ├── show_display.rs
│   │   │   ├── show_types.rs
│   │   │   └── templates.rs
│   │   ├── sync/               # Cloud sync operations
│   │   │   ├── mod.rs
│   │   │   ├── admin_user.rs
│   │   │   ├── interactive.rs
│   │   │   ├── prompt.rs
│   │   │   ├── skills.rs
│   │   │   └── content/
│   │   │       ├── mod.rs
│   │   │       └── display.rs
│   │   ├── templates/          # Cloud templates
│   │   │   ├── mod.rs
│   │   │   ├── checkout.rs
│   │   │   └── oauth.rs
│   │   └── tenant/             # Tenant management
│   │       ├── mod.rs
│   │       ├── create.rs
│   │       ├── crud.rs
│   │       ├── docker.rs
│   │       ├── rotate.rs
│   │       ├── select.rs
│   │       └── validation.rs
│   │
│   ├── content/                # Content edit (standalone)
│   │   └── edit.rs
│   │
│   ├── core/                   # systemprompt core [...]
│   │   ├── mod.rs
│   │   ├── content/            # Content management
│   │   │   ├── mod.rs
│   │   │   ├── delete.rs
│   │   │   ├── delete_source.rs
│   │   │   ├── ingest.rs
│   │   │   ├── list.rs
│   │   │   ├── popular.rs
│   │   │   ├── publish.rs
│   │   │   ├── search.rs
│   │   │   ├── show.rs
│   │   │   ├── status.rs
│   │   │   ├── types.rs
│   │   │   ├── verify.rs
│   │   │   ├── analytics/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── campaign.rs
│   │   │   │   ├── clicks.rs
│   │   │   │   └── journey.rs
│   │   │   ├── files/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── featured.rs
│   │   │   │   ├── link.rs
│   │   │   │   ├── list.rs
│   │   │   │   └── unlink.rs
│   │   │   └── link/
│   │   │       ├── mod.rs
│   │   │       ├── delete.rs
│   │   │       ├── generate.rs
│   │   │       ├── list.rs
│   │   │       ├── performance.rs
│   │   │       └── show.rs
│   │   ├── contexts/           # Context management
│   │   │   ├── mod.rs
│   │   │   ├── create.rs
│   │   │   ├── delete.rs
│   │   │   ├── edit.rs
│   │   │   ├── list.rs
│   │   │   ├── new.rs
│   │   │   ├── resolve.rs
│   │   │   ├── show.rs
│   │   │   ├── types.rs
│   │   │   └── use_context.rs
│   │   ├── files/              # File management
│   │   │   ├── mod.rs
│   │   │   ├── config.rs
│   │   │   ├── delete.rs
│   │   │   ├── list.rs
│   │   │   ├── search.rs
│   │   │   ├── show.rs
│   │   │   ├── stats.rs
│   │   │   ├── types.rs
│   │   │   ├── upload.rs
│   │   │   ├── validate.rs
│   │   │   └── ai/
│   │   │       ├── mod.rs
│   │   │       ├── count.rs
│   │   │       ├── list.rs
│   │   │       └── show.rs
│   │   └── skills/             # Skill management
│   │       ├── mod.rs
│   │       ├── create.rs
│   │       ├── delete.rs
│   │       ├── edit.rs
│   │       ├── list.rs
│   │       ├── show.rs
│   │       ├── status.rs
│   │       ├── sync.rs
│   │       └── types.rs
│   │
│   ├── infrastructure/         # systemprompt infra [...]
│   │   ├── mod.rs
│   │   ├── db/                 # Database operations
│   │   │   ├── mod.rs
│   │   │   ├── admin.rs
│   │   │   ├── helpers.rs
│   │   │   ├── introspect.rs
│   │   │   ├── query.rs
│   │   │   ├── schema.rs
│   │   │   └── types.rs
│   │   ├── jobs/               # Scheduled jobs
│   │   │   ├── mod.rs
│   │   │   ├── cleanup_logs.rs
│   │   │   ├── cleanup_sessions.rs
│   │   │   ├── disable.rs
│   │   │   ├── enable.rs
│   │   │   ├── helpers.rs
│   │   │   ├── history.rs
│   │   │   ├── list.rs
│   │   │   ├── run.rs
│   │   │   ├── show.rs
│   │   │   └── types.rs
│   │   ├── logs/               # Log management
│   │   │   ├── mod.rs
│   │   │   ├── audit.rs
│   │   │   ├── audit_display.rs
│   │   │   ├── cleanup.rs
│   │   │   ├── delete.rs
│   │   │   ├── duration.rs
│   │   │   ├── export.rs
│   │   │   ├── search.rs
│   │   │   ├── search_queries.rs
│   │   │   ├── shared.rs
│   │   │   ├── show.rs
│   │   │   ├── stream.rs
│   │   │   ├── summary.rs
│   │   │   ├── types.rs
│   │   │   ├── view.rs
│   │   │   ├── request/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── list.rs
│   │   │   │   ├── show.rs
│   │   │   │   └── stats.rs
│   │   │   ├── tools/
│   │   │   │   ├── mod.rs
│   │   │   │   ├── list.rs
│   │   │   │   └── queries.rs
│   │   │   └── trace/
│   │   │       ├── mod.rs
│   │   │       ├── ai_artifacts.rs
│   │   │       ├── ai_display.rs
│   │   │       ├── ai_mcp.rs
│   │   │       ├── display.rs
│   │   │       ├── json.rs
│   │   │       ├── list.rs
│   │   │       ├── show.rs
│   │   │       └── summary.rs
│   │   ├── services/           # Service management
│   │   │   ├── mod.rs
│   │   │   ├── cleanup.rs
│   │   │   ├── restart.rs
│   │   │   ├── serve.rs
│   │   │   ├── start.rs
│   │   │   ├── status.rs
│   │   │   └── stop.rs
│   │   └── system/             # System operations
│   │       ├── mod.rs
│   │       ├── login.rs
│   │       └── types.rs
│   │
│   ├── plugins/                # systemprompt plugins [...]
│   │   ├── mod.rs
│   │   ├── config.rs           # Plugin config
│   │   ├── list.rs             # List plugins
│   │   ├── run.rs              # Run plugin
│   │   ├── show.rs             # Show plugin details
│   │   ├── types.rs            # Plugin output types
│   │   ├── validate.rs         # Validate plugin
│   │   ├── capabilities/       # Capability introspection
│   │   │   ├── mod.rs
│   │   │   ├── jobs.rs
│   │   │   ├── llm_providers.rs
│   │   │   ├── roles.rs
│   │   │   ├── schemas.rs
│   │   │   ├── templates.rs
│   │   │   └── tools.rs
│   │   └── mcp/                # MCP server management
│   │       ├── mod.rs
│   │       ├── call.rs
│   │       ├── list.rs
│   │       ├── list_packages.rs
│   │       ├── logs.rs
│   │       ├── status.rs
│   │       ├── tools.rs
│   │       ├── types.rs
│   │       └── validate.rs
│   │
│   ├── shared/                 # Shared command utilities
│   │   ├── mod.rs
│   │   └── validation.rs
│   │
│   └── web/                    # systemprompt web [...]
│       ├── mod.rs
│       ├── types.rs
│       ├── validate.rs
│       ├── assets/
│       │   ├── mod.rs
│       │   ├── list.rs
│       │   └── show.rs
│       ├── content_types/
│       │   ├── mod.rs
│       │   ├── create.rs
│       │   ├── delete.rs
│       │   ├── edit.rs
│       │   ├── list.rs
│       │   └── show.rs
│       ├── sitemap/
│       │   ├── mod.rs
│       │   ├── generate.rs
│       │   └── show.rs
│       └── templates/
│           ├── mod.rs
│           ├── create.rs
│           ├── delete.rs
│           ├── edit.rs
│           ├── list.rs
│           └── show.rs
│
├── presentation/               # Output rendering
│   ├── mod.rs
│   ├── renderer.rs             # Format-aware rendering
│   ├── state.rs                # Render state management
│   └── widgets.rs              # Terminal widgets
│
├── routing/                    # Remote execution
│   ├── mod.rs
│   └── remote.rs               # SSE streaming for remote CLI
│
└── shared/                     # Cross-cutting infrastructure
    ├── mod.rs
    ├── command_result.rs       # CommandResult<T> wrapper
    ├── docker.rs               # Docker utilities
    ├── parsers.rs              # CLI value parsers
    ├── paths.rs                # Path utilities
    ├── process.rs              # Process management
    ├── profile.rs              # Profile resolution
    ├── project.rs              # Project detection
    └── web.rs                  # Web utilities
```

---

## Module Explanations

### Core Modules

| Module | Purpose |
|--------|---------|
| `lib.rs` | Main entry point with `pub async fn run()`, command routing, and initialization |
| `bootstrap.rs` | Orchestrates initialization sequence: profile → credentials → secrets → paths → validation |
| `cli_settings.rs` | Global configuration: `CliConfig`, `OutputFormat`, `VerbosityLevel` |
| `requirements.rs` | `HasRequirements` trait for commands to declare initialization needs |
| `session.rs` | Session lifecycle: JWT tokens, context management, persistence |

### Command Groups

| Group | Command | Purpose |
|-------|---------|---------|
| `admin` | `agents` | Create, manage, and orchestrate AI agents |
| `admin` | `config` | View and modify system configuration |
| `admin` | `session` | Manage CLI sessions and profiles |
| `admin` | `setup` | Interactive setup wizard |
| `admin` | `users` | User administration (CRUD, roles, bans) |
| `analytics` | `*` | Usage metrics, costs, traffic analysis |
| `build` | `*` | Build MCP servers and web assets |
| `cloud` | `auth` | OAuth login, logout, whoami |
| `cloud` | `deploy` | Deploy to cloud infrastructure |
| `cloud` | `profile` | Profile CRUD operations |
| `cloud` | `sync` | Sync content, skills, users to cloud |
| `cloud` | `tenant` | Tenant management (create, select, rotate) |
| `core` | `content` | Content ingestion, publishing, analytics |
| `core` | `contexts` | Context management for sessions |
| `core` | `files` | File upload, management, AI processing |
| `core` | `skills` | Skill definitions and sync |
| `infrastructure` | `db` | Database queries and administration |
| `infrastructure` | `jobs` | Scheduled job management |
| `infrastructure` | `logs` | Log viewing, search, export, traces |
| `infrastructure` | `services` | Start, stop, restart services |
| `plugins` | `capabilities` | Introspect registered capabilities |
| `plugins` | `mcp` | MCP server management and tool execution |
| `web` | `*` | Web assets, templates, sitemap management |

### Shared Infrastructure

| Module | Purpose |
|--------|---------|
| `presentation/` | Output rendering: table, JSON, YAML, widgets |
| `routing/` | Remote CLI execution via SSE streaming |
| `shared/` | Utilities: parsers, paths, docker, profile resolution |

---

## Architecture

### Command Requirements System

Commands declare initialization needs via the `HasRequirements` trait:

```rust
pub enum Requirements {
    None,               // Standalone operation
    ProfileOnly,        // Needs profile loaded
    ProfileAndSecrets,  // Needs profile + secrets
    Full,               // Needs everything + database
}
```

### Bootstrap Sequence

1. Parse CLI arguments (clap)
2. Build `CliConfig` from args/env
3. Check command requirements
4. `resolve_profile()` → CLI override → env var → session
5. `init_profile()` → `init_credentials()` → `init_secrets()` → `init_paths()` → `run_validation()`

### Output System

All commands return `CommandResult<T>`:

```rust
CommandResult::table(data)
    .with_title("Title")
    .with_hints(json!({ "columns": [...] }))
```

Artifact types: `Table`, `List`, `Card`, `Text`, `CopyPasteText`, `Chart`, `Form`, `Dashboard`

---

## Dual-Mode Operation

Every command supports:

| Mode | Audience | Behavior |
|------|----------|----------|
| Interactive | Humans | Rich prompts, confirmations, colored output |
| Non-Interactive | Agents | All inputs via flags, JSON output, no prompts |

```bash
# Interactive
systemprompt admin agents create

# Non-interactive
systemprompt --non-interactive --json admin agents create --name myagent
```

---

## Standard Flags

| Flag | Short | Purpose |
|------|-------|---------|
| `--yes` | `-y` | Skip confirmation |
| `--dry-run` | | Preview without executing |
| `--force` | | Override safety checks |
| `--json` | | JSON output |
| `--yaml` | | YAML output |
| `--non-interactive` | | Disable prompts |
| `--quiet` | | Minimal output |
| `--verbose` | | Detailed output |

---

## Related Documentation

- [Validation Checklist](./validation.md)
- [Rust Standards](/instructions/rust/rust.md)
- [Compliance Status](./status.md)

## Installation

```bash
cargo install systemprompt-cli
```

Or add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-cli = "0.0.1"
```

## License

FSL-1.1-ALv2 - See [LICENSE](../../LICENSE) for details.
