# 21 — CLI End-to-End Testing Roadmap

This is a **planning document** for CLI testing. It sequences work into phases and waves, with enough detail for an agent to execute wave-by-wave without further research.

---

## Context

The CLI crate (`crates/entry/cli/`) was explicitly skipped during the Phase 4-5 coverage campaign (report 14, report 20). It sits at **1.8% line coverage** across **56.7K LOC** — the largest untested surface in the codebase.

**Current state:**

- **191 existing unit tests** in `crates/tests/unit/entry/cli/` covering `cli_settings`, `descriptor`, and `shared` (parsers, profile, project, command_result)
- **Zero argument-parsing tests** — no tests verify that `Cli::try_parse_from()` accepts or rejects command strings
- **Zero command-execution tests** — no tests call any `execute()` function
- **8 top-level command groups** with 130+ leaf subcommands total

**Why this matters:** The CLI is the primary user interface. Argument parsing bugs, descriptor flag mismatches, and filesystem command regressions are all caught only in production today.

---

## Current CLI Architecture

### Bootstrap Flow

```
Cli::parse() → build_cli_config() → CommandDescriptor check
  → if desc.profile():  resolve_and_display_profile() → enforce_routing_policy()
  → if desc.secrets():  init_secrets()
  → if desc.paths():    init_paths() → run_validation()
  → if desc.database(): AppContext::new() (connects to DB)
  → dispatch_command()
```

Entry point: `crates/entry/cli/src/lib.rs::run()` calls `args::Cli::parse()` then `dispatch_command()`.

### CommandDescriptor Flags

Defined in `crates/entry/cli/src/descriptor.rs`. A 6-bit flag set that controls the bootstrap path:

| Preset | Flags | Used By |
|--------|-------|---------|
| `NONE` | `0b000000` | `admin setup`, `session show/list/logout`, `cloud auth/init/tenant/profile` |
| `PROFILE_ONLY` | `0b000001` | `build core/mcp`, `cloud status/restart/domain`, `plugins mcp logs/list/list-packages`, `plugins run` |
| `PROFILE_AND_SECRETS` | `0b000011` | `session login/switch`, `cloud deploy/sync/secrets` |
| `PROFILE_SECRETS_AND_PATHS` | `0b000111` | `admin config`, `web *`, `core hooks/plugins/agents list|show|validate`, `core skills list|show`, `infra services`, `cloud sync local` |
| `FULL` | `0b011111` | All DB-dependent commands (content, files, contexts, artifacts, infra db/jobs/logs, admin users/agents, analytics) |
| `FULL.with_skip_validation()` | `0b111111` | `core skills create`, `infra jobs run/list`, `analytics *` |

### Command Taxonomy (130+ leaf commands)

| Group | Subgroups | Leaf Commands |
|-------|-----------|---------------|
| **core** | agents (4), artifacts (2), content (12+subs), contexts (7), files (9+subs), hooks (2), plugins (4), skills (7) | ~47 |
| **infra** | services (6), db (13), jobs (9), logs (12+subs) | ~40 |
| **admin** | users (15+subs), agents (13), config (9+subs), setup (1), session (5) | ~43 |
| **cloud** | auth (3), init (1), tenant (8), profile (5), deploy (1), status (1), restart (1), sync (4), secrets (4), dockerfile (1), db (12), domain (3) | ~44 |
| **analytics** | overview (1), conversations, agents, tools, requests, sessions, content, traffic, costs | ~18+ |
| **web** | content-types (5), templates (5), assets (2), sitemap (2), validate (1) | ~15 |
| **plugins** | list, show, run, validate, config, capabilities, mcp (7 subs) | ~13 |
| **build** | core, mcp | ~2 |

---

## Prerequisites (Phase 0) — Test Harness

Before writing any tests, build a reusable test harness. All harness code lives in the existing test crate.

### Files to Create

**`crates/tests/unit/entry/cli/src/harness.rs`** — Test environment and helpers:

```
pub struct TestCliEnv {
    pub tempdir: TempDir,
    pub project_root: PathBuf,
    pub profiles_dir: PathBuf,
}
```

Responsibilities:
- Creates a `tempdir` with `.systemprompt/profiles/<name>/profile.yaml` fixture
- Provides `write_profile(name, content)` and `write_secrets(name, content)` helpers
- Sets `SYSTEMPROMPT_PROJECT_ROOT` env var to tempdir path (scoped per test)
- Provides `create_services_dir(agents, mcp_servers)` for commands that read loader configs
- Provides `create_skill_yaml(agent_name, skill_name, content)` for skill/agent filesystem tests

**`crates/tests/unit/entry/cli/src/parse_helpers.rs`** — Clap parse wrappers:

```
pub fn parse_ok(args: &[&str]) -> args::Cli
pub fn parse_err(args: &[&str]) -> clap::Error
pub fn parse_command(args: &[&str]) -> args::Commands
```

These wrap `Cli::try_parse_from(std::iter::once("systemprompt").chain(args.iter().copied()))`.

### Dependencies to Add

In `crates/tests/unit/entry/cli/Cargo.toml`, add:
- `clap = { workspace = true }` (needed for `try_parse_from`)
- `tokio = { workspace = true, features = ["rt-multi-thread", "macros"] }` (already in dev-deps, may need in deps for async execute tests)

### Update `lib.rs`

Add modules:
```rust
#[cfg(test)]
mod harness;
#[cfg(test)]
mod parse_helpers;
#[cfg(test)]
mod arg_parsing;
#[cfg(test)]
mod filesystem_commands;
```

### Visibility Requirements

The following items in `systemprompt-cli` must be made `pub` (currently `pub(crate)` or private):

- `crate::args::Cli` — needed for `try_parse_from()` in test crate
- `crate::args::Commands` — needed to inspect parsed variant
- `crate::args::build_cli_config` — needed to construct `CliConfig` from parsed `Cli`
- `crate::args::reconstruct_args` — already `pub`
- All command enum types are already re-exported through `pub use commands::{admin, analytics, build, cloud, core, infrastructure, plugins, web}` in `lib.rs`

Check each and bump visibility only where needed. Minimal production changes.

---

## Phase 1: Argument Parsing (~160 tests)

**Goal:** Verify every command path parses correctly via `Cli::try_parse_from()`, test invalid args produce errors, and verify descriptor flags per command.

**Test file:** `crates/tests/unit/entry/cli/src/arg_parsing/mod.rs` with submodules per group.

### 1A: Top-Level Parsing (~20 tests)

**File:** `crates/tests/unit/entry/cli/src/arg_parsing/top_level.rs`

Tests:
- `parse_no_args` — `Cli::try_parse_from(["systemprompt"])` succeeds, `command` is `None`
- `parse_help_flag` — `--help` produces `ErrorKind::DisplayHelp`
- `parse_version_flag` — `--version` produces `ErrorKind::DisplayVersion`
- `parse_unknown_command` — `Cli::try_parse_from(["systemprompt", "bogus"])` fails
- `parse_verbose_flag` — `-v` sets `verbosity.verbose = true`
- `parse_quiet_flag` — `-q` sets `verbosity.quiet = true`
- `parse_debug_flag` — `--debug` sets `verbosity.debug = true`
- `parse_verbose_quiet_conflict` — `-v -q` fails (clap conflict)
- `parse_json_flag` — `--json` sets `output.json = true`
- `parse_yaml_flag` — `--yaml` sets `output.yaml = true`
- `parse_json_yaml_conflict` — `--json --yaml` fails
- `parse_no_color_flag` — `--no-color` sets `display.no_color = true`
- `parse_non_interactive_flag` — `--non-interactive`
- `parse_database_url_flag` — `--database-url postgres://...`
- `parse_profile_flag` — `--profile myprofile`
- `build_cli_config_verbose` — test `build_cli_config()` with verbose flag
- `build_cli_config_json` — test `build_cli_config()` with json flag
- `build_cli_config_quiet` — test `build_cli_config()` with quiet flag
- `build_cli_config_no_color` — test `build_cli_config()` sets `ColorMode::Never`
- `build_cli_config_profile_override` — test profile is passed through

### 1B: Core Group Parsing (~24 tests)

**File:** `crates/tests/unit/entry/cli/src/arg_parsing/core.rs`

Test each subcommand parses:
- `core agents list`, `core agents show <name>`, `core agents sync`, `core agents validate`
- `core artifacts list`, `core artifacts show <id>`
- `core skills list`, `core skills show <name>`, `core skills create --name x`, `core skills edit <name>`, `core skills delete <name>`, `core skills status`, `core skills sync`
- `core contexts list`, `core contexts show <id>`, `core contexts create`, `core contexts edit`, `core contexts delete`, `core contexts use <id>`, `core contexts new`
- `core hooks list`, `core hooks validate`
- `core plugins list`, `core plugins show <name>`, `core plugins validate`, `core plugins generate`
- Invalid: `core bogus`, `core agents`, `core skills`

### 1C: Infra Group Parsing (~28 tests)

**File:** `crates/tests/unit/entry/cli/src/arg_parsing/infra.rs`

- `infra services start`, `infra services start --all`, `infra services start --api`, `infra services start agent <id>`, `infra services start mcp <name>`
- `infra services stop`, `infra services stop --force`, `infra services stop agent <id>`, `infra services stop mcp <name>`
- `infra services restart api`, `infra services restart agent <id>`, `infra services restart mcp <name> --build`, `infra services restart --failed`
- `infra services status`, `infra services status --detailed --health`
- `infra services cleanup`, `infra services cleanup --yes --dry-run`
- `infra services serve`, `infra services serve --foreground`
- `infra db query "SELECT 1"`, `infra db execute "DELETE..."`, `infra db tables`, `infra db describe users`, `infra db info`, `infra db migrate`, `infra db status`, `infra db validate`, `infra db count users`, `infra db indexes`, `infra db size`
- `infra jobs list`, `infra jobs show <name>`, `infra jobs run <name>`, `infra jobs history`, `infra jobs enable <name>`, `infra jobs disable <name>`
- `infra logs view --tail 20`, `infra logs search "error"`, `infra logs stream`, `infra logs export --format json`, `infra logs summary`, `infra logs show <id>`

### 1D: Admin Group Parsing (~20 tests)

**File:** `crates/tests/unit/entry/cli/src/arg_parsing/admin.rs`

- `admin users list`, `admin users show <id>`, `admin users search <term>`, `admin users create`, `admin users delete <id>`, `admin users count`, `admin users export`, `admin users stats`, `admin users merge`
- `admin agents list`, `admin agents show <name>`, `admin agents validate`, `admin agents create`, `admin agents edit <name>`, `admin agents delete <name>`, `admin agents status`, `admin agents logs`, `admin agents registry`, `admin agents message`, `admin agents task`, `admin agents tools`, `admin agents run`
- `admin config show`, `admin config list`, `admin config validate`
- `admin setup`
- `admin session show`, `admin session list`, `admin session switch <name>`, `admin session login`, `admin session logout`

### 1E: Cloud Group Parsing (~20 tests)

**File:** `crates/tests/unit/entry/cli/src/arg_parsing/cloud.rs`

- `cloud auth login`, `cloud auth logout`, `cloud auth whoami`
- `cloud init`, `cloud init --force`
- `cloud tenant create`, `cloud tenant list`, `cloud tenant show`, `cloud tenant delete`, `cloud tenant edit`
- `cloud profile list`, `cloud profile show`, `cloud profile delete <name>`, `cloud profile edit`
- `cloud deploy`, `cloud deploy --skip-push --dry-run`
- `cloud status`, `cloud restart`, `cloud restart --yes`
- `cloud sync push`, `cloud sync pull`, `cloud sync local skills`
- `cloud secrets sync`, `cloud secrets set KEY=VAL`, `cloud secrets unset KEY`
- `cloud dockerfile`
- `cloud db migrate --profile p`, `cloud db query --profile p "SELECT 1"`, `cloud db backup --profile p`
- `cloud domain set example.com`, `cloud domain status`, `cloud domain remove`

### 1F: Analytics, Web, Plugins, Build Parsing (~24 tests)

**File:** `crates/tests/unit/entry/cli/src/arg_parsing/analytics.rs`
- `analytics overview`, `analytics conversations`, `analytics agents`, `analytics tools`, `analytics requests`, `analytics sessions`, `analytics content`, `analytics traffic`, `analytics costs`

**File:** `crates/tests/unit/entry/cli/src/arg_parsing/web.rs`
- `web content-types list`, `web content-types show <name>`, `web content-types create`, `web content-types edit`, `web content-types delete`
- `web templates list`, `web templates show <name>`, `web templates create`, `web templates edit`, `web templates delete`
- `web assets list`, `web assets show <path>`
- `web sitemap show`, `web sitemap generate`
- `web validate`

**File:** `crates/tests/unit/entry/cli/src/arg_parsing/plugins.rs`
- `plugins list`, `plugins show <name>`, `plugins run <name>`, `plugins validate`, `plugins config <name>`, `plugins capabilities`
- `plugins mcp list`, `plugins mcp status <name>`, `plugins mcp validate <name>`, `plugins mcp logs <name>`, `plugins mcp list-packages`, `plugins mcp tools <name>`, `plugins mcp call <name>`

**File:** `crates/tests/unit/entry/cli/src/arg_parsing/build.rs`
- `build core`, `build mcp`

### 1G: Descriptor Flag Tests (~24 tests)

**File:** `crates/tests/unit/entry/cli/src/arg_parsing/descriptors.rs`

For each command variant, verify `DescribeCommand::descriptor()` returns the expected preset:

- `Commands::Build(_)` returns `PROFILE_ONLY`
- `Commands::Admin(AdminCommands::Setup(_))` returns `NONE`
- `Commands::Admin(AdminCommands::Session(SessionCommands::Show))` returns `NONE`
- `Commands::Admin(AdminCommands::Session(SessionCommands::Login(_)))` returns `PROFILE_AND_SECRETS`
- `Commands::Admin(AdminCommands::Config(_))` returns `PROFILE_SECRETS_AND_PATHS`
- `Commands::Web(_)` returns `PROFILE_SECRETS_AND_PATHS`
- `Commands::Core(CoreCommands::Hooks(_))` returns `PROFILE_SECRETS_AND_PATHS`
- `Commands::Core(CoreCommands::Skills(SkillsCommands::List(_)))` returns `PROFILE_SECRETS_AND_PATHS`
- `Commands::Core(CoreCommands::Skills(SkillsCommands::Create(_)))` returns `FULL.with_skip_validation()`
- `Commands::Infra(InfraCommands::Services(_))` returns `PROFILE_SECRETS_AND_PATHS`
- `Commands::Infra(InfraCommands::Jobs(JobsCommands::Run(_)))` returns `FULL.with_skip_validation()`
- `Commands::Analytics(_)` returns `FULL.with_skip_validation()`
- `Commands::Cloud(CloudCommands::Init { .. })` returns `NONE`
- `Commands::Cloud(CloudCommands::Deploy { .. })` returns `PROFILE_AND_SECRETS`
- `Commands::Cloud(CloudCommands::Status)` returns `PROFILE_ONLY`
- `Commands::Cloud(CloudCommands::Sync { command: Some(SyncCommands::Local(_)) })` returns `PROFILE_SECRETS_AND_PATHS`
- `Commands::Plugins(PluginsCommands::List(_))` returns `PROFILE_ONLY.with_remote_eligible()`
- `Commands::Plugins(PluginsCommands::Run(_))` returns `PROFILE_ONLY`
- `Commands::Plugins(PluginsCommands::Mcp(McpCommands::Logs(_)))` returns `PROFILE_ONLY`
- `Commands::Plugins(PluginsCommands::Mcp(McpCommands::Status(_)))` returns `FULL`
- Each descriptor preset: test `profile()`, `secrets()`, `paths()`, `database()`, `remote_eligible()`, `skip_validation()` boolean accessors

**Estimated total Phase 1: ~160 tests**

---

## Phase 2: Filesystem-Only Commands (~265 tests)

**Goal:** Test commands that read/write the filesystem but do not require a database connection. These commands have descriptors `NONE`, `PROFILE_ONLY`, or `PROFILE_SECRETS_AND_PATHS` (without `FLAG_DATABASE`).

Each test uses `TestCliEnv` to create a tempdir with the required fixtures, then calls the command's `execute()` function directly (not through the full bootstrap path).

### 2A: Core Skills — Filesystem Operations (~35 tests)

**File:** `crates/tests/unit/entry/cli/src/filesystem_commands/core_skills.rs`

Commands: `skills list`, `skills show`

Fixtures needed: tempdir with `services/<agent>/skills/<skill>.yaml` files

Tests:
- `list_empty_services_dir` — no agents, returns empty list
- `list_single_agent_single_skill` — one skill YAML, returns it
- `list_multiple_agents_multiple_skills` — verify aggregation
- `list_with_disabled_skill` — skill with `enabled: false`
- `show_existing_skill` — returns full YAML content
- `show_nonexistent_skill` — returns error
- `show_with_json_output` — verify JSON serialization via `CliConfig`
- Edge cases: malformed YAML, missing fields, empty skill file, skill with all optional fields

### 2B: Core Agents — Filesystem Operations (~30 tests)

**File:** `crates/tests/unit/entry/cli/src/filesystem_commands/core_agents.rs`

Commands: `agents list`, `agents show`, `agents validate`

Fixtures: tempdir with `services/<agent>/agent.yaml`

Tests:
- `list_no_agents` — empty result
- `list_single_agent` — returns agent config
- `list_multiple_agents` — sorted output
- `show_existing_agent` — full config returned
- `show_nonexistent` — error
- `validate_valid_config` — passes
- `validate_invalid_config` — reports errors
- `validate_missing_required_fields` — reports which fields
- JSON output variants for list/show/validate

### 2C: Core Hooks (~15 tests)

**File:** `crates/tests/unit/entry/cli/src/filesystem_commands/core_hooks.rs`

Commands: `hooks list`, `hooks validate`

Fixtures: hook definitions in plugin manifests

Tests:
- `list_no_hooks` — empty
- `list_hooks_across_plugins` — aggregation
- `validate_valid_hooks` — passes
- `validate_invalid_hook_definition` — reports errors
- JSON output variants

### 2D: Core Plugins (~20 tests)

**File:** `crates/tests/unit/entry/cli/src/filesystem_commands/core_plugins.rs`

Commands: `plugins list`, `plugins show`, `plugins validate`, `plugins generate`

Fixtures: `services/` directory with `manifest.yaml` files

Tests:
- `list_no_plugins` — empty
- `list_discovered_plugins` — finds them
- `show_existing_plugin` — full details
- `show_nonexistent` — error
- `validate_valid` — passes
- `validate_missing_manifest` — reports error
- `generate_output` — produces Claude Code plugin format

### 2E: Admin Config (~25 tests)

**File:** `crates/tests/unit/entry/cli/src/filesystem_commands/admin_config.rs`

Commands: `admin config show`, `admin config list`, `admin config validate`, `admin config rate-limits`, `admin config server`, `admin config runtime`, `admin config security`, `admin config paths`, `admin config provider`

Fixtures: profile with `profile.yaml`, `secrets.json`

Tests:
- `show_displays_overview` — reads profile config
- `list_finds_config_files` — lists all YAML/JSON in profile
- `validate_valid_config` — passes
- `validate_missing_secrets` — reports error
- `rate_limits_show` — displays configured limits
- `server_show` — displays server host/port
- `runtime_show` — displays runtime config
- `security_show` — displays security settings
- `paths_show` — displays configured paths
- `provider_show` — displays AI provider config
- JSON output for each

### 2F: Admin Session (~20 tests)

**File:** `crates/tests/unit/entry/cli/src/filesystem_commands/admin_session.rs`

Commands: `session show`, `session list`, `session logout` (descriptor=NONE, no bootstrap needed)

Fixtures: `.systemprompt/` with session file and multiple profiles

Tests:
- `show_no_session` — shows no active session
- `show_with_active_session` — shows profile name
- `list_no_profiles` — empty list
- `list_multiple_profiles` — shows all
- `logout_existing_session` — removes session file
- `logout_no_session` — no-op or warning

### 2G: Cloud Init / Tenant / Profile — Filesystem (~30 tests)

**File:** `crates/tests/unit/entry/cli/src/filesystem_commands/cloud_filesystem.rs`

Commands with `NONE` descriptor: `cloud init`, `cloud tenant list/show/edit`, `cloud profile list/show/delete/edit`

Fixtures: `.systemprompt/cloud/tenants.json`, `.systemprompt/profiles/*/profile.yaml`

Tests:
- `init_creates_structure` — creates `.systemprompt/` dirs
- `init_force_overwrites` — `--force` flag
- `init_idempotent` — running twice is safe
- `tenant_list_empty` — no tenants
- `tenant_list_populated` — shows tenants from `tenants.json`
- `tenant_show_existing` — shows details
- `tenant_show_missing` — error
- `tenant_edit_updates_field` — modifies `tenants.json`
- `profile_list_empty` — no profiles
- `profile_list_populated` — shows profiles
- `profile_show_existing` — reads profile.yaml
- `profile_show_missing` — error
- `profile_delete_existing` — removes profile dir
- `profile_delete_missing` — error
- `profile_delete_requires_confirmation` — without `--yes`
- `profile_edit_set_host` — updates profile.yaml
- `profile_edit_set_port` — updates profile.yaml
- JSON output variants

### 2H: Cloud Dockerfile (~10 tests)

**File:** `crates/tests/unit/entry/cli/src/filesystem_commands/cloud_dockerfile.rs`

Command: `cloud dockerfile` — pure filesystem, calls `dockerfile::generate_dockerfile_content()`

Fixtures: project root with `Cargo.toml` and extension discovery structure

Tests:
- `generate_basic_dockerfile` — produces valid Dockerfile content
- `generate_with_extensions` — includes extension-specific lines
- `generate_empty_project` — minimal Dockerfile
- `json_output_wraps_content` — JSON mode wraps in `DockerfileOutput`

### 2I: Web Commands (~30 tests)

**File:** `crates/tests/unit/entry/cli/src/filesystem_commands/web_commands.rs`

Commands: `web content-types list/show/create/edit/delete`, `web templates list/show/create/edit/delete`, `web assets list/show`, `web sitemap show/generate`, `web validate`

All web commands use descriptor `PROFILE_SECRETS_AND_PATHS` but no database flag.

Fixtures: profile with paths pointing to tempdir, content-type YAMLs, template files, asset files

Tests per subgroup:
- Content types (5): list empty/populated, show existing/missing, create/edit/delete
- Templates (5): list empty/populated, show existing/missing, create/edit/delete
- Assets (3): list empty/populated, show existing/missing
- Sitemap (3): show config, generate creates file, generate with content
- Validate (2): valid config, invalid config
- JSON output variants (12)

### 2J: Build Commands (~10 tests)

**File:** `crates/tests/unit/entry/cli/src/filesystem_commands/build_commands.rs`

Commands: `build core`, `build mcp` — descriptor `PROFILE_ONLY`

These invoke `cargo build` subprocess, so tests verify:
- Command construction (args passed correctly)
- Error when not in project directory
- JSON output format
- Note: actual cargo invocation should be mocked or tested as error-path-only (no Cargo.toml fixture)

### 2K: Plugins Extension Commands (~20 tests)

**File:** `crates/tests/unit/entry/cli/src/filesystem_commands/plugins_commands.rs`

Commands: `plugins list`, `plugins show`, `plugins validate`, `plugins config`, `plugins capabilities`, `plugins mcp list`, `plugins mcp list-packages`

Fixtures: extension discovery paths

Tests:
- `list_no_extensions` — empty
- `list_discovered_extensions` — finds them
- `show_extension_details` — returns metadata
- `show_unknown_extension` — error
- `validate_all_valid` — passes
- `validate_with_issues` — reports problems
- `config_shows_extension_config` — reads config
- `capabilities_aggregates_all` — shows all capabilities
- `mcp_list_servers` — lists configured MCP servers
- `mcp_list_packages` — lists available packages

### 2L: Infra Jobs List (~10 tests)

**File:** `crates/tests/unit/entry/cli/src/filesystem_commands/infra_jobs.rs`

Command: `infra jobs list` — descriptor `FULL.with_skip_validation()` but the `list::execute()` function itself just returns registered jobs without DB

Tests:
- `list_returns_registered_jobs` — non-empty list of known job types
- `list_json_output` — JSON format
- Verify each known job has name, schedule, description

**Estimated total Phase 2: ~265 tests**

---

## Phase 3: Shared Utilities (~90 tests)

**Goal:** Test remaining untested shared utilities. Some already have tests; this covers gaps.

### 3A: Analytics Shared (~30 tests)

**File:** `crates/tests/unit/entry/cli/src/shared/analytics_shared.rs`

Module: `crates/entry/cli/src/commands/analytics/shared/`

Functions to test from `time.rs`:
- `parse_duration("1h")` → 3600s, `parse_duration("30m")`, `parse_duration("7d")`, invalid strings
- `parse_since("1h")` → DateTime, `parse_until("2026-04-01")`
- `parse_time_range(since, until)` — valid/invalid combos
- `format_duration_ms(12345)` → human readable
- `format_period_label(period)` — for each period variant
- `format_timestamp(ts)` — ISO format
- `truncate_to_period(dt, period)` — hourly/daily/weekly/monthly

Functions from `output.rs`:
- `format_number(1234567)` → "1,234,567"
- `format_percent(0.1234)` → "12.34%"
- `format_cost(1500000)` → "$1.50" (microdollars)
- `format_tokens(1000000)` → "1M"
- `format_change(old, new)` → "+15.2%" or "-3.1%"

Functions from `export.rs`:
- `resolve_export_path(base, name)` → valid path
- `ensure_export_dir(path)` → creates dirs
- `export_to_csv(data, path)` → writes CSV
- `CsvBuilder` methods

### 3B: Presentation Layer (~25 tests)

**File:** `crates/tests/unit/entry/cli/src/shared/presentation.rs`

Module: `crates/entry/cli/src/presentation/`

- `renderer.rs` — output rendering functions
- `widgets.rs` — terminal UI widgets
- `state.rs` — presentation state management

Tests:
- Renderer produces valid output for each format (text, JSON, YAML)
- Widget construction and formatting
- State transitions

Note: these may require visibility bumps on `presentation` module items.

### 3C: Docker / Process Utilities (~15 tests)

**File:** `crates/tests/unit/entry/cli/src/shared/docker_process.rs`

Module: `crates/entry/cli/src/shared/docker.rs`, `process.rs`

- Docker command construction
- Process PID file handling
- Signal helpers (if any pure functions exist)
- Error cases: docker not found, process not running

### 3D: Text Utilities (~10 tests)

**File:** `crates/tests/unit/entry/cli/src/shared/text_utils.rs`

Module: `crates/entry/cli/src/shared/text.rs`

- `truncate_with_ellipsis("long string", 10)` → "long st..."
- Empty string, exact length, shorter than max
- Unicode handling

### 3E: Paths Utilities (~10 tests)

**File:** `crates/tests/unit/entry/cli/src/shared/paths_utils.rs`

Module: `crates/entry/cli/src/shared/paths.rs`

- Path resolution logic
- Default path construction
- Edge cases: missing dirs, relative paths

**Estimated total Phase 3: ~90 tests**

---

## Phase 4: Error-Path-Only Tests (~30 tests)

**Goal:** Test commands that require external services (DB, running processes, network) by verifying they fail gracefully when services are unavailable.

### 4A: DB-Requiring Commands — Graceful Failure (~20 tests)

**File:** `crates/tests/unit/entry/cli/src/error_paths/db_commands.rs`

For each DB-requiring command group, verify that calling `execute()` without a database produces a meaningful error (not a panic).

Test commands:
- `core content list` — "Failed to connect to database" or similar
- `core files list` — same
- `core contexts list` — same
- `core artifacts list` — same
- `core skills status` / `core skills sync` — same
- `core agents sync` — same
- `infra db query "SELECT 1"` — same
- `infra db tables` — same
- `infra logs view` — same
- `admin users list` — same
- `admin agents status` — same

Each test calls the execute function with a valid `CliConfig` but no database, asserts `Result::Err` with an error message containing recognizable text.

### 4B: Process-Requiring Commands — Graceful Failure (~10 tests)

**File:** `crates/tests/unit/entry/cli/src/error_paths/process_commands.rs`

- `infra services status` — no running services, should return empty/error
- `infra services stop` — nothing to stop
- `admin agents status` — no running agents
- `admin agents logs` — no running agents
- `plugins mcp status <name>` — MCP server not running
- `plugins mcp logs <name>` — no logs available
- `plugins run <name>` — extension binary not found

**Estimated total Phase 4: ~30 tests**

---

## Phase 5: Explicitly Deferred

The following are **not in scope** for this roadmap:

### DB-Dependent Integration Tests (Future: report 22)

Commands that require a live database connection for meaningful testing:

| Group | Commands | Why Deferred |
|-------|----------|-------------|
| core content | list, show, search, edit, delete, delete-source, popular, verify, status, link, analytics, files | All use `AppContext::new()` → DB pool |
| core files | list, show, upload, delete, search, stats, ai | DB pool required |
| core contexts | all 7 commands | DB pool required |
| core artifacts | list, show | DB pool required |
| core skills | status, sync, create, edit, delete | DB for sync operations |
| core agents | sync | DB for sync |
| infra db | all 13 commands | Obviously DB-dependent |
| infra logs | all 12 commands | Logs stored in DB |
| infra jobs | show, run, history, enable, disable, cleanup-sessions, log-cleanup | DB for job state |
| infra services | start, serve (trigger DB migrations) | Starts DB connection |
| admin users | all 15+ commands | User table queries |
| admin agents | status, logs, registry, message, task, tools, run | Need running services or DB |
| analytics | all 18+ commands | All query DB |
| cloud sync push/pull | Cloud API + sync token | External service |
| cloud deploy | Cloud API | External service |
| cloud auth login | OAuth flow | External service |
| cloud secrets | Cloud API | External service |
| cloud db | all 12 commands | Remote DB |
| cloud domain | all 3 commands | Cloud API |
| cloud status/restart | Cloud API | External service |

These require either:
1. A test database (integration test infrastructure)
2. Mock HTTP services (for cloud API commands)
3. Running process infrastructure (for services/agent commands)

### Interactive Commands

Commands using `dialoguer` for interactive input:
- `cloud tenant` (no subcommand) — interactive menu
- `cloud profile` (no subcommand) — interactive menu
- `cloud sync` (no subcommand) — interactive menu
- `cloud tenant create` — interactive type selection
- `admin setup` — setup wizard

These require `--non-interactive` mode or are untestable without terminal mocking.

---

## Wave Execution Model

Each wave runs 3 parallel agents. Each agent works on a disjoint set of files.

| Wave | Phase | Agent 1 | Agent 2 | Agent 3 | Expected Tests |
|------|-------|---------|---------|---------|---------------|
| **W1** | P0+P1 | Harness (`harness.rs`, `parse_helpers.rs`) + visibility bumps | Top-level + Core + Infra parsing (1A, 1B, 1C) | Admin + Cloud + remaining parsing (1D, 1E, 1F) | ~20 (harness) + ~72 + ~64 = ~156 |
| **W2** | P1+P2 | Descriptor tests (1G) + Core skills/agents filesystem (2A, 2B) | Core hooks/plugins + Admin config/session (2C, 2D, 2E, 2F) | Cloud filesystem + Dockerfile (2G, 2H) | ~24+65 + ~80 + ~40 = ~209 |
| **W3** | P2 | Web + Build + Plugins filesystem (2I, 2J, 2K) | Infra jobs + Analytics shared (2L, 3A) | Presentation + Docker/Process + Text + Paths (3B, 3C, 3D, 3E) | ~60 + ~40 + ~60 = ~160 |
| **W4** | P4 | Error paths DB (4A) | Error paths Process (4B) | Sweep: fix any failing tests, add edge cases | ~20 + ~10 + ~10 = ~40 |

**Total: 4 waves, 12 agent assignments, ~565 new tests**

---

## Verification Protocol

After each wave, the executing agent must:

1. **Build check:**
   ```bash
   cargo build --manifest-path crates/tests/Cargo.toml -p systemprompt-cli-tests 2>&1 | tail -20
   ```

2. **Run tests:**
   ```bash
   cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-cli-tests -- --nocapture 2>&1 | tail -40
   ```

3. **Count tests:**
   ```bash
   cargo test --manifest-path crates/tests/Cargo.toml -p systemprompt-cli-tests 2>&1 | grep "test result"
   ```

4. **No production regressions:**
   ```bash
   cargo build --workspace 2>&1 | tail -5
   ```

5. **Commit with test count in message:**
   ```
   test: add CLI E2E wave N — <count> new tests
   ```

---

## Expected Outcomes

| Metric | Before | After |
|--------|--------|-------|
| CLI test count | 191 | ~756 |
| CLI test files | 3 modules (cli_settings, descriptor, shared) | ~25+ modules |
| Argument parsing coverage | 0% | ~95% of all 130+ commands |
| Descriptor flag coverage | partial (existing descriptor.rs) | 100% of all variants |
| Filesystem command coverage | 0% | ~30 commands tested |
| Error path coverage | 0% | ~20 commands verified graceful failure |
| Total project tests | 8,535 | ~9,100 |
| CLI line coverage (est.) | 1.8% | ~12-15% (parsing + filesystem paths) |

The remaining ~85% of CLI line coverage requires database integration tests (report 22) and process/service integration tests.

---

## File Index

All new files created by this roadmap:

| File | Phase | Contents |
|------|-------|----------|
| `crates/tests/unit/entry/cli/src/harness.rs` | P0 | TestCliEnv, profile fixtures |
| `crates/tests/unit/entry/cli/src/parse_helpers.rs` | P0 | Clap parse wrappers |
| `crates/tests/unit/entry/cli/src/arg_parsing/mod.rs` | P1 | Module declarations |
| `crates/tests/unit/entry/cli/src/arg_parsing/top_level.rs` | P1 | Top-level flag tests |
| `crates/tests/unit/entry/cli/src/arg_parsing/core.rs` | P1 | Core group parsing |
| `crates/tests/unit/entry/cli/src/arg_parsing/infra.rs` | P1 | Infra group parsing |
| `crates/tests/unit/entry/cli/src/arg_parsing/admin.rs` | P1 | Admin group parsing |
| `crates/tests/unit/entry/cli/src/arg_parsing/cloud.rs` | P1 | Cloud group parsing |
| `crates/tests/unit/entry/cli/src/arg_parsing/analytics.rs` | P1 | Analytics parsing |
| `crates/tests/unit/entry/cli/src/arg_parsing/web.rs` | P1 | Web parsing |
| `crates/tests/unit/entry/cli/src/arg_parsing/plugins.rs` | P1 | Plugins parsing |
| `crates/tests/unit/entry/cli/src/arg_parsing/build.rs` | P1 | Build parsing |
| `crates/tests/unit/entry/cli/src/arg_parsing/descriptors.rs` | P1 | Descriptor flag verification |
| `crates/tests/unit/entry/cli/src/filesystem_commands/mod.rs` | P2 | Module declarations |
| `crates/tests/unit/entry/cli/src/filesystem_commands/core_skills.rs` | P2 | Skills list/show |
| `crates/tests/unit/entry/cli/src/filesystem_commands/core_agents.rs` | P2 | Agents list/show/validate |
| `crates/tests/unit/entry/cli/src/filesystem_commands/core_hooks.rs` | P2 | Hooks list/validate |
| `crates/tests/unit/entry/cli/src/filesystem_commands/core_plugins.rs` | P2 | Plugins list/show/validate/generate |
| `crates/tests/unit/entry/cli/src/filesystem_commands/admin_config.rs` | P2 | Config show/list/validate + subs |
| `crates/tests/unit/entry/cli/src/filesystem_commands/admin_session.rs` | P2 | Session show/list/logout |
| `crates/tests/unit/entry/cli/src/filesystem_commands/cloud_filesystem.rs` | P2 | Init, tenant, profile filesystem ops |
| `crates/tests/unit/entry/cli/src/filesystem_commands/cloud_dockerfile.rs` | P2 | Dockerfile generation |
| `crates/tests/unit/entry/cli/src/filesystem_commands/web_commands.rs` | P2 | All web subcommands |
| `crates/tests/unit/entry/cli/src/filesystem_commands/build_commands.rs` | P2 | Build core/mcp |
| `crates/tests/unit/entry/cli/src/filesystem_commands/plugins_commands.rs` | P2 | Extension discovery commands |
| `crates/tests/unit/entry/cli/src/filesystem_commands/infra_jobs.rs` | P2 | Jobs list |
| `crates/tests/unit/entry/cli/src/shared/analytics_shared.rs` | P3 | Time, output, export utils |
| `crates/tests/unit/entry/cli/src/shared/presentation.rs` | P3 | Renderer, widgets, state |
| `crates/tests/unit/entry/cli/src/shared/docker_process.rs` | P3 | Docker, process utils |
| `crates/tests/unit/entry/cli/src/shared/text_utils.rs` | P3 | Text truncation |
| `crates/tests/unit/entry/cli/src/shared/paths_utils.rs` | P3 | Path resolution |
| `crates/tests/unit/entry/cli/src/error_paths/mod.rs` | P4 | Module declarations |
| `crates/tests/unit/entry/cli/src/error_paths/db_commands.rs` | P4 | DB-absent error tests |
| `crates/tests/unit/entry/cli/src/error_paths/process_commands.rs` | P4 | Process-absent error tests |
