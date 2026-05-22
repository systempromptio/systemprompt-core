# CLI Reference

The complete command surface of the `systemprompt` CLI. The binary groups commands into eight top-level domains: `core`, `infra`, `admin`, `cloud`, `analytics`, `web`, `plugins`, and `build`.

Command structure is defined in `crates/entry/cli/src/args.rs` and the modules under `crates/entry/cli/src/commands/`.

## Synopsis

```
systemprompt [GLOBAL OPTIONS] <DOMAIN> <SUBCOMMAND> [ARGS]
```

## Global options

These apply to every command (`crates/entry/cli/src/args.rs`).

| Option | Effect |
|--------|--------|
| `-v`, `--verbose` | Increase verbosity. |
| `-q`, `--quiet` | Suppress output. Conflicts with `--verbose`. |
| `--debug` | Enable debug logging. |
| `--json` | Emit JSON output. |
| `--yaml` | Emit YAML output. Conflicts with `--json`. |
| `--no-color` | Disable coloured output. |
| `--non-interactive` | Disable interactive prompts. |
| `--database-url <URL>` | Use a direct database URL, bypassing the profile. Reads `SYSTEMPROMPT_DATABASE_URL`. |
| `--profile <NAME>` | Select a profile, overriding the active session. |
| `--version` | Print the version. |
| `--help` | Print help. |

Configuration comes from the active profile unless `--profile` or `--database-url` overrides it.

## core — skills, content, files, contexts

`crates/entry/cli/src/commands/core/`

| Subcommand | Notable commands | Purpose |
|------------|------------------|---------|
| `core artifacts` | — | Artifact inspection and debugging. |
| `core content` | — | Content management and analytics. |
| `core files` | — | File management and uploads. |
| `core contexts` | `list`, `show`, `create`, `rename`, `delete`, `use`, `create-and-use` | Context management. |
| `core skills` | `list`, `show` | Skill management and database sync. |
| `core plugins` | — | Plugin management and marketplace generation. |
| `core hooks` | — | Hook validation and inspection. |

```bash
systemprompt core skills list
```

## infra — services, db, jobs, logs

`crates/entry/cli/src/commands/infrastructure/`

### infra services

Service lifecycle management.

| Command | Notable flags | Purpose |
|---------|---------------|---------|
| `infra services start [TARGET]` | `--all`, `--api`, `--agents`, `--mcp`, `--foreground`, `--skip-migrate`, `--kill-port-process` | Start API, agents, and MCP servers. `TARGET` may be `agent <name>` or `mcp <name>`. |
| `infra services stop [TARGET]` | `--all`, `--api`, `--agents`, `--mcp`, `--force` | Stop services gracefully. |
| `infra services restart [TARGET]` | `--failed`, `--agents`, `--mcp` | Restart services. `TARGET` may be `api`, `agent <name>`, or `mcp <name>`. |
| `infra services status` | `--detailed`, `--json`, `--health` | Show service status. |
| `infra services cleanup` | `-y`/`--yes`, `--dry-run` | Clean up orphaned processes and stale entries. |
| `infra services serve` | `--foreground`, `--kill-port-process` | Start the API server (also starts agents and MCP servers). |

```bash
systemprompt infra services start --all
```

### infra db

Database operations (`crates/entry/cli/src/commands/infrastructure/db/commands.rs`).

| Command | Purpose |
|---------|---------|
| `infra db query <SQL>` | Run a read-only query (`--limit`, `--offset`). |
| `infra db execute <SQL>` | Run a write (INSERT/UPDATE/DELETE). |
| `infra db tables` | List tables with row counts and sizes. |
| `infra db describe <TABLE>` | Describe a table's schema. |
| `infra db info` | Show database information. |
| `infra db status` | Show connection status. |
| `infra db migrate` | Run pending migrations. |
| `infra db migrate-down <EXT>` | Revert the most recent migrations for an extension. |
| `infra db migrate-plan` | Show pending migrations without writing. |
| `infra db migrate-status` | Detailed migration status (applied, pending, drift). |
| `infra db migrate-repair` | Repair migration checksum drift. |
| `infra db migrate-mark-applied` | Record a migration as applied without running it. |
| `infra db migrate-squash` | Squash an extension's migrations into a baseline. |
| `infra db migrations` | Show migration status and history. |
| `infra db validate` | Validate the schema against expected tables. |
| `infra db count <TABLE>` | Row count for a table. |
| `infra db indexes` | List indexes. |
| `infra db size` | Database and table sizes. |
| `infra db doctor` | Diff live schema against extension declarations. |
| `infra db assign-admin <USER>` | Assign the admin role to a user. |

```bash
systemprompt infra db query "SELECT * FROM users LIMIT 10"
```

### infra jobs

| Command | Purpose |
|---------|---------|
| `infra jobs list` | List available jobs. |
| `infra jobs show <JOB>` | Show details about a job. |
| `infra jobs run <JOB>` | Run a scheduled job manually. |
| `infra jobs history` | View job execution history. |
| `infra jobs enable <JOB>` | Enable a job. |
| `infra jobs disable <JOB>` | Disable a job. |

```bash
systemprompt infra jobs list
```

### infra logs

| Command | Purpose |
|---------|---------|
| `infra logs traces` | Debug execution traces. |
| `infra logs requests` | Inspect AI requests. |
| `infra logs tools` | List and search MCP tool executions. |
| `infra logs cleanup` | Clean up old log entries. |

```bash
systemprompt infra logs traces
```

## admin — users, agents, config, setup, session

`crates/entry/cli/src/commands/admin/`

| Subcommand | Notable commands | Purpose |
|------------|------------------|---------|
| `admin users` | `list`, `show`, `search`, `create`, `update`, `delete`, `count`, `export`, `stats`, `merge`, `roles`, `sessions`, `bans`, `webauthn` | User management and IP banning. |
| `admin agents` | `list`, `show`, `validate`, `create`, `edit`, `delete`, `status`, `logs`, `running`, `send`, `task`, `tools`, `run` | Agent management and A2A interaction. |
| `admin config` | — | Configuration management and rate limits. |
| `admin setup` | — | Interactive setup wizard for local development. |
| `admin bootstrap` | — | Idempotently ensure the platform admin user exists. |
| `admin session` | `info` (alias `current`), `switch`, `list`, `create`, `remove` | CLI session and profile switching. |
| `admin bridge` | — | Bridge helper enrollment (device certs, exchange codes). |
| `admin access-control` | — | Access-control baseline operations (DB to YAML export). |
| `admin keys` | — | RSA signing-key generation for the JWT plane. |

```bash
systemprompt admin agents status my-agent
```

## cloud — deployment, sync, tenants

`crates/entry/cli/src/commands/cloud/`

| Subcommand | Notable flags / commands | Purpose |
|------------|--------------------------|---------|
| `cloud auth` | `login`, `logout` | Authenticate with systemprompt.io Cloud via OAuth. |
| `cloud init` | `--force` | Initialize project structure. |
| `cloud tenant` | `create`, `show`, `delete`, `edit`, `rotate-credentials`, `cancel` | Manage tenants (local or cloud). |
| `cloud profile` | — | Manage profiles. |
| `cloud deploy` | `--skip-push`, `-p`/`--profile`, `--no-sync`, `-y`/`--yes`, `--dry-run` | Deploy to systemprompt.io Cloud. |
| `cloud status` | — | Check cloud deployment status. |
| `cloud restart` | `--tenant`, `-y`/`--yes` | Restart a tenant machine. |
| `cloud sync` | — | Sync between local and cloud environments. |
| `cloud secrets` | — | Manage secrets for a cloud tenant. |
| `cloud dockerfile` | — | Generate a Dockerfile from discovered extensions. |
| `cloud db` | — | Cloud database operations. |
| `cloud domain` | — | Manage custom domain and TLS certificates. |

```bash
systemprompt cloud auth login
```

## analytics — metrics reporting

`crates/entry/cli/src/commands/analytics/`

| Subcommand | Purpose |
|------------|---------|
| `analytics overview` | Dashboard overview of all analytics. |
| `analytics conversations` | Conversation analytics. |
| `analytics agents` | Agent performance analytics. |
| `analytics tools` | Tool usage analytics. |
| `analytics requests` | AI request analytics. |
| `analytics sessions` | Session analytics. |
| `analytics content` | Content performance analytics. |
| `analytics traffic` | Traffic analytics. |
| `analytics costs` | Cost analytics. |

```bash
systemprompt analytics overview
```

## web — web service configuration

`crates/entry/cli/src/commands/web/`

| Subcommand | Purpose |
|------------|---------|
| `web content-types` | Manage content types. |
| `web templates` | Manage templates. |
| `web assets` | List and inspect assets. |
| `web sitemap` | Sitemap operations. |
| `web validate` | Validate web configuration. Exits non-zero on errors. |

```bash
systemprompt web validate
```

## plugins — extensions and MCP servers

`crates/entry/cli/src/commands/plugins/`

| Subcommand | Purpose |
|------------|---------|
| `plugins list` | List all discovered extensions. |
| `plugins show <NAME>` | Show detailed extension information. |
| `plugins run <NAME> [ARGS]` | Run a CLI extension command. |
| `plugins validate` | Validate extension dependencies and configurations. |
| `plugins config <NAME>` | Show extension configuration. |
| `plugins capabilities` | List capabilities across all extensions. |
| `plugins mcp` | MCP server management. |

```bash
systemprompt plugins list
```

## build — build extensions

`crates/entry/cli/src/commands/build/`

| Subcommand | Purpose |
|------------|---------|
| `build core` | Build the Rust workspace (systemprompt-core). |
| `build mcp` | Build MCP extensions. |

```bash
systemprompt build mcp
```
