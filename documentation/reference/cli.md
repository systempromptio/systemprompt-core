# CLI Reference

The complete command surface of the `systemprompt` CLI. The binary groups commands into eight top-level domains: `core`, `infra`, `admin`, `cloud`, `analytics`, `web`, `plugins`, and `build`.

The command tree is assembled at startup from two sources: the kernel groups named in `crates/entry/cli/src/runner/args/mod.rs` together with the kernel's built-in contributed groups (`build`), and the command groups the composition's members contribute through `Contributions::command(CommandDecl::group::<T>())` (or `group_under`/`leaf_under` to attach under a kernel group such as `core`, `web` or `admin`). A contributed group is a `CliCommandGroup` that carries its name, help and the same bootstrap descriptor kernel commands carry: whether it needs a profile, secrets, paths or a database, how it routes against a cloud profile, whether it destroys data, and whether it accepts `--database-url`; it runs as `run(&dyn CommandHost) -> CommandOutput`. The registry (`crates/entry/cli/src/kernel/registry.rs`) admits every contribution before the profile is loaded: two contributions of one path, one that shadows a kernel command or a subcommand of another group, one under a parent that does not exist, one claiming a reserved CLI path (`RESERVED_CLI_PATHS`) without being that path's core owner type, or one requesting a kernel grant not listed for its path stops the binary with an error naming the contributors. A member command whose owner the profile disables is refused before it runs, and a member command needs a profile: it is refused on a run given only `--database-url`.

## Synopsis

```
systemprompt [GLOBAL OPTIONS] <DOMAIN> <SUBCOMMAND> [ARGS]
```

## Global options

These apply to every command (`crates/entry/cli/src/runner/args/mod.rs`).

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
| `infra services stop [TARGET]` | `--all`, `--api`, `--agents`, `--mcp`, `--force` | Stop services gracefully. A process is signalled only when it carries this installation's identity marker; any other process on a service port is reported and left running. |
| `infra services restart [TARGET]` | `--failed`, `--agents`, `--mcp` | Restart services. `TARGET` may be `api`, `agent <name>`, or `mcp <name>`. |
| `infra services status` | `--detailed`, `--json`, `--health` | Show service status. |
| `infra services cleanup` | `-y`/`--yes`, `--dry-run` | Clean up orphaned processes and stale entries. |
| `infra services serve` | `--foreground`, `--kill-port-process` | Start the API server (also starts agents and MCP servers). On Linux and macOS the process re-executes itself once, keeping its PID, to carry the API server identity marker that `stop`, `restart api` and `cleanup` verify. `--kill-port-process` signals a port holder that is not a verified API server only after an interactive confirmation naming its PID. |

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
| `infra jobs run <JOB>` | Run a scheduled job manually. A manual run is an explicit operator act and runs the job whatever its enabled flag. |
| `infra jobs history` | View job execution history. |
| `infra jobs enable <JOB>` | Enable a job. |
| `infra jobs disable <JOB>` | Stop a job's scheduled runs; the setting survives restarts. |
| `infra jobs prune [--dry-run] [--yes]` | List the `scheduled_jobs` rows no job linked into this binary names and delete them after confirmation. A job whose extension the profile disables is kept. Run it from a binary that links every capability the cluster runs. |

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
| `admin config` | — | Configuration management, rate limits, and declared config planes (`planes list`, `diff`, `export`, `apply`, see [config planes](../concepts/config-authority.md)). |
| `admin setup` | — | Interactive setup wizard for local development. |
| `admin bootstrap` | — | Idempotently ensure the platform admin user exists. |
| `admin session` | `info` (alias `current`), `switch`, `list`, `create`, `remove` | CLI session and profile switching. |
| `admin bridge` | — | Bridge helper enrollment (device certs, exchange codes). |
| `admin access-control` | — | Access-control baseline operations (DB to YAML export). |
| `admin keys` | — | RSA signing-key generation for the JWT plane. |

```bash
systemprompt admin agents status my-agent
```

## cloud — deployment, backup, tenants

`crates/entry/cloud/src/cli/` (`systemprompt-cloud-entry`, `cli` feature). The cloud member contributes the group through `Contributions::command`; a binary whose composition lacks that member has no `cloud` command.

| Subcommand | Notable flags / commands | Purpose |
|------------|--------------------------|---------|
| `cloud auth` | `login`, `logout`, `whoami`, `admin-user` | Authenticate with systemprompt.io Cloud via OAuth; `admin-user` projects the authenticated cloud user as an admin into local profile databases. |
| `cloud init` | `--force` | Initialize project structure. |
| `cloud tenant` | `create`, `add`, `list`, `show`, `delete`, `edit`, `rotate-credentials` | Manage tenants: systemprompt.io-managed (synced from the control plane) or self-managed (`add --provider kubernetes\|gke\|docker_local`). |
| `cloud profile` | — | Manage profiles. |
| `cloud deploy` | `--skip-push`, `-p`/`--profile`, `--check` | Deploy to systemprompt.io Cloud. Runs the deploy preflight first and blocks on failure; `--check` runs the preflight only. Deploys replace the running container: runtime files created inside it are not preserved — run `cloud backup` first if you need a copy. |
| `cloud backup` | `-p`/`--profile`, `-o`/`--output`, `--list` | Download the tenant's runtime `services/` tree to a standalone directory (never the project's own `services/`). `--list` shows the remote manifest without downloading. |
| `cloud doctor` | `-p`/`--profile` | Run the deploy preflight standalone (profile, secrets, signing key, provider credentials, extension config). Exits non-zero on failure. |
| `cloud status` | — | Check cloud deployment status. |
| `cloud restart` | `--tenant`, `-y`/`--yes` | Restart a tenant machine. |
| `cloud secrets` | — | Manage secrets for a cloud tenant. |
| `cloud dockerfile` | — | Generate a Dockerfile from discovered extensions. |
| `cloud db` | — | Cloud database operations. |
| `cloud domain` | — | Manage custom domain and TLS certificates. |

```bash
systemprompt cloud auth login
```

## analytics — metrics reporting

`crates/entry/analytics/src/cli/` (`systemprompt-analytics-entry`, feature `cli`)

A group contributed by the analytics HTTP member. Every subcommand is read-only, needs a profile and its database (a run given only `--database-url` is refused), and writes an `--export` file on the local machine even when the profile routes to a cloud tenant. The same member contributes `core content popular` (feature `content-commands`).

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
| `plugins config <NAME>` | Show extension configuration. |
| `plugins capabilities jobs\|schemas\|roles` | List jobs, schemas or roles across the admitted members. |
| `plugins seams [--member TEXT]` | List the admitted composition, member by member, with what each contributes. |
| `plugins mcp` | MCP server management. |

```bash
systemprompt plugins list
```

### Admitted composition

A capability exists only in a binary whose composition includes the member that contributes it, and only when the profile does not disable that member. `plugins seams` prints one row per admitted member and contribution kind (provided and required state, slots, schemas, migrations, seeds, jobs, job-run scopes, retention, user purges, orphan sweeps, projections, run origins, subject providers, policies, gateway guards, runner services, readiness checks, diagnostics, metrics, roles, configuration sections and planes):

| Column | Meaning |
|--------|---------|
| `member` | Extension id of the admitted member. |
| `standing` | Whether the member is a core member or an install member. |
| `contribution` | Contribution kind. |
| `count`, `entries` | Number of entries; for schemas, jobs, roles and configs, their table names, job names, role names or config prefixes. |

`--member` keeps only members whose id or name contains the text. A member the profile disables is not admitted and has no rows. A profile setting that needs a contribution no admitted member makes is refused when the composition is admitted, before any command runs, so run a command against the deployment profile before shipping a custom binary.

```bash
systemprompt plugins seams
systemprompt plugins seams --member gateway --json
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
