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

# systemprompt-cli

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/entry-cli.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/entry-cli.svg">
    <img alt="systemprompt-cli terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/entry-cli.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-cli.svg?style=flat-square)](https://crates.io/crates/systemprompt-cli)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-cli?style=flat-square)](https://docs.rs/systemprompt-cli)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Unified CLI for systemprompt.io: agent orchestration, MCP governance, analytics, profiles, cloud deployment, and self-hosted operations. Every command supports both human-friendly interactive mode and agent-friendly non-interactive mode.

**Layer**: Entry — application boundary. Binary: `systemprompt`. Library: `systemprompt_cli`. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

The CLI exposes the systemprompt.io platform through eight top-level domains:

| Domain | Purpose |
|--------|---------|
| `core` | Skills, content, files, contexts, plugins, hooks, artefacts |
| `infra` | Service lifecycle, database, jobs, log streaming |
| `admin` | Users, agents, configuration, session, setup wizard, bridge enrolment, access-control baseline |
| `cloud` | Authentication, tenants, profiles, deploy, sync, secrets, custom domains, Dockerfile, database |
| `analytics` | Overview, conversations, agents, tools, requests, sessions, content, traffic, costs |
| `web` | Content types, templates, assets, sitemap, validation |
| `plugins` | Extension discovery, configuration, execution, capability inspection, MCP server management |
| `build` | Build core workspace, build MCP extensions |

## Architecture

```
src/
├── lib.rs                 # Entry point: pub async fn run(), command routing
├── args.rs                # Top-level Commands enum (clap parsing)
├── bootstrap.rs           # Profile → credentials → secrets → paths → validation
├── cli_settings.rs        # CliConfig, OutputFormat, VerbosityLevel
├── descriptor.rs          # CommandDescriptor: declares initialisation needs per command
├── environment.rs         # Process environment resolution
├── interactive.rs         # Interactive menu mode
├── paths.rs               # CLI-local path helpers
├── session/               # Session lifecycle (JWT, context, persistence)
├── presentation/          # Output rendering: tables, JSON, YAML, widgets
├── routing/               # Local vs remote (SSE) command execution
├── shared/                # Cross-cutting utilities (parsers, paths, docker, profile)
│
└── commands/
    ├── mod.rs             # Domain module re-exports
    ├── admin/             # Users, agents, config, session, setup, bridge, access-control
    ├── analytics/         # Overview, conversations, agents, tools, requests,
    │                      # sessions, content, traffic, costs
    ├── build/             # Core workspace and MCP extension builds
    ├── cloud/             # Auth, init, tenant, profile, deploy, status, restart,
    │                      # sync, secrets, dockerfile, db, domain, templates
    ├── core/              # Artefacts, content, files, contexts, skills, plugins, hooks
    ├── infrastructure/    # Services, db, jobs, logs
    ├── plugins/           # List, show, run, validate, config, capabilities, mcp
    ├── web/               # Content types, templates, assets, sitemap, validate
    └── shared/            # Cross-domain command helpers
```

The top-level `Commands` enum in `src/args.rs` dispatches to per-domain `*Commands` subcommand enums declared in each `commands/<domain>/mod.rs`.

### Core Modules

| Module | Purpose |
|--------|---------|
| `lib.rs` | `pub async fn run()` entry point, top-level routing, output finalisation |
| `args.rs` | clap-derived `Cli` and `Commands` definitions |
| `bootstrap.rs` | Initialisation sequence: profile → credentials → secrets → paths → validation |
| `cli_settings.rs` | `CliConfig`, `OutputFormat`, `VerbosityLevel` |
| `descriptor.rs` | `CommandDescriptor` constants (`NONE`, `PROFILE_ONLY`, `FULL`, etc.) declared per command |
| `session/` | JWT session tokens, active context, on-disk persistence |
| `routing/` | `ExecutionTarget` (local vs remote SSE streaming) |
| `presentation/` | Format-aware renderer, render state, terminal widgets |
| `shared/` | `CommandResult<T>`, docker utilities, value parsers, process helpers, profile/project detection |

### Command Requirements

Each command variant declares a `CommandDescriptor` indicating what bootstrap state it needs:

- `NONE` — standalone (no profile, no secrets, no database)
- `PROFILE_ONLY` — profile loaded
- `PROFILE_AND_SECRETS` — profile and secrets loaded
- `FULL` — profile, secrets, paths, validation, database pool

`bootstrap.rs` walks the descriptor and short-circuits unneeded steps.

### Output System

All commands return `CommandResult<T>`:

```rust
CommandResult::table(data)
    .with_title("Title")
    .with_hints(json!({ "columns": [...] }))
```

Artefact variants: `Table`, `List`, `Card`, `Text`, `CopyPasteText`, `Chart`, `Form`, `Dashboard`. The renderer picks a representation based on `--json`, `--yaml`, or interactive TTY.

## Usage

```toml
[dependencies]
systemprompt-cli = "0.14.0"
```

```bash
cargo install systemprompt-cli
```

## Dual-Mode Operation

Every command supports two modes:

| Mode | Audience | Behaviour |
|------|----------|-----------|
| Interactive | Humans | Prompts, confirmations, coloured output |
| Non-interactive | Agents | All inputs via flags, structured output, no prompts |

```bash
# Interactive
systemprompt admin agents create

# Non-interactive
systemprompt --non-interactive --json admin agents create --name myagent
```

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

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial licence. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-cli)** · **[docs.rs](https://docs.rs/systemprompt-cli)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Entry layer · Own how your organisation uses AI.</sub>

</div>
