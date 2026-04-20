# Changelog

## [0.2.3] - 2026-04-20

### Fixed
- **Local-trial profiles no longer require cloud credentials for local-capable operations.** `create_new_session` now routes `SessionKey::Tenant("local_*")` profiles to the local-session path, and `resolve_local_user_email` falls back to `admin@localhost.dev` (matching `demo/00-preflight.sh`) when no `session_email_hint` is provided. Downstream commands previously broken on a fresh `just setup-local` clone (`admin agents tools`, `plugins mcp tools/call`, `core contexts list`, agent trace lookups) now work without running `systemprompt cloud auth login`. Cloud-only entrypoints (`cloud sync`, `cloud tenant select`, `admin session login`, `admin session switch`) are unchanged.

### Changed
- `bootstrap.rs` `is_local_profile` predicate now delegates to `Profile::is_local_trial()` — the duplicated 12-line inline check is gone.

## [0.2.0] - 2026-04-15

### Changed
- `cloud profile show` uses `ConfigLoader::load()` (replacing `EnhancedConfigLoader::from_env()?.load()`).

## [0.1.24] - 2026-04-14

### Fixed
- Local-only profiles (`cloud.tenant_id` prefixed `local_`, or `cloud.validation: warn`/`skip`) no longer surface a `✗ Cloud credential error: Credentials not initialized` line on startup; the message is downgraded to a `debug!` log so local evals run clean.

## [0.1.23] - 2026-04-14

### Fixed
- `admin agents message` and `admin agents task` now send A2A v1.0.0 method names (`SendMessage`, `SendStreamingMessage`, `GetTask`) — previously every call was rejected by the server with `Unsupported method`, causing blocking calls to hang until `--timeout` elapsed

## [0.1.21] - 2026-04-02

### Fixed
- Validate cached CLI sessions against the database before reuse — stale sessions are now detected and removed
- Use `ApiPaths` constant for default agent endpoint in `agents create`

### Changed
- Session login now connects to database before checking cached session, enabling DB validation

## [0.1.18] - 2026-03-27

### Added
- Tenant cancel subscription, show, list, edit, and delete commands

### Changed
- Upgrade to Rust 2024 edition
- Split large CLI modules into focused files across tenant, secrets, services, and config commands

## [0.1.17a] - 2026-02-26

### Fixed
- Rename `total_cents` to `total_cost_microdollars` in analytics overview (field contained microdollars, not cents)
- Rename `avg_cost_per_request_cents` to `avg_cost_per_request_microdollars` in cost summary output
- Fix `format_cost()` to divide by 1,000,000 (microdollars) instead of 100 (cents)

## [0.1.17] - 2026-02-19

### Added
- `systemprompt core agents list` command with `--enabled`/`--disabled` filters
- `systemprompt core agents show <name>` command for agent details
- `systemprompt core agents sync` command for bidirectional disk-database sync
- `systemprompt core agents validate [name]` command for config validation

## [0.1.16] - 2026-02-19

### Changed
- Refactor hooks CLI to use `HookEvent::ALL_VARIANTS` and `matchers_for_event()` instead of hardcoded event name strings
- `count_hooks` in plugin show now uses `HookEvent` enum iteration instead of manual field chaining

## [0.1.15] - 2026-02-18

### Changed
- Consolidate duplicate `SkillConfig`, `ParsedSkill`, `strip_frontmatter`, and `parse_skill_from_config` into shared types
- Replace `unwrap_or("")` with explicit error handling in skills list and plugin agent generation
- Add `tracing::warn!` for silently skipped YAML parse errors in agent generation
- Replace magic `"config.yaml"` and `"index.md"` string literals with shared constants

## [0.1.14] - 2026-02-17

### Added
- `systemprompt core plugins list` command with `--enabled`/`--disabled` filters
- `systemprompt core plugins show <id>` command for plugin details
- `systemprompt core plugins validate [id]` command for config validation
- `systemprompt core plugins generate [--id <id>]` command for marketplace artifact generation
- `systemprompt core hooks list` command to list hooks across plugins
- `systemprompt core hooks validate` command for hook config validation
- Marketplace JSON generation in `plugins generate` for Claude Code plugin distribution

### Changed
- Split `plugins/generate.rs` (571 lines) into 6 focused modules under `generate/`
- Replace magic string comparisons with `ComponentSource`/`ComponentFilter` enum matching
- Extract `DEFAULT_AGENT_TOOLS` and `PLUGIN_ROOT_VAR` constants
- Add `tracing::warn!` for silent error paths in marketplace and hook scanning
- Introduce `PluginGenerateContext` struct to reduce function parameter count

### Removed
- Remove `systemprompt core playbooks` subcommand group (create, edit, delete, list, show, sync)

## [0.1.13] - 2026-02-11

### Fixed
- Skip external database URL routing when running on Fly.io (container uses internal URL)

## [0.1.12] - 2026-02-11

### Added
- Auto-generate sync tokens during deploy if not configured, saving to profile secrets
- `external_database_url` field in generated cloud profile secrets
- Cloud profiles with `external_db_access` now route CLI commands to the external database URL

### Changed
- Refactor session login into reusable `login_for_profile()` for profile-specific authentication
- `admin session switch` now loads secrets directly from target profile instead of relying on global bootstrap
- `cloud db` commands use `Secrets::load_from_path()` and `effective_database_url()` instead of manual JSON parsing

## [0.1.11] - 2026-02-11

### Fixed
- `cloud db` commands now prefer `external_database_url` from secrets.json, falling back to `database_url`
- `--database-url` global flag now works with `cloud db` subcommands instead of being rejected

## [0.1.10] - 2026-02-10

### Fixed
- Fix cloud tenant creation failing to retrieve database credentials after checkout
- CLI now waits for backend provisioning to complete (via SSE with polling fallback) before fetching secrets, instead of making a single immediate call that races with async infrastructure setup

### Changed
- Version bump for workspace consistency with analytics and content routing changes

## [0.1.4] - 2026-02-07

### Fixed
- Ensure `systemprompt_admin` PostgreSQL role exists during local tenant creation
- Docker `POSTGRES_USER` only creates the role on first volume initialization; reusing an existing container or volume would fail with `role "systemprompt_admin" does not exist`
- Tenant creation now explicitly verifies and creates the admin role via the `postgres` superuser before any database operations

## [0.1.3] - 2026-02-03

### Added
- Cloud activity tracking for CLI login/logout events via `POST /api/v1/activity`
- `ApiPaths::ACTIVITY_EVENT_LOGIN` and `ApiPaths::ACTIVITY_EVENT_LOGOUT` constants

### Changed
- `cloud auth logout` command is now async to support activity reporting

## [0.1.2] - 2026-02-03

### Added
- Initialize logging with database pool in `admin agents run` command

### Changed
- Analytics cost displays now use `cost_microdollars` for sub-cent precision
- Regenerated SQLx offline query cache

## [0.1.1] - 2026-02-03

### Added
- Support nested playbook directory structures (e.g., `domain/agents/operations.md`)
- Auto-load user email from credentials for `admin session login` (no longer requires `--email` flag)
- Playbook IDs now map underscores to path separators (`domain_agents_operations` → `domain/agents/operations.md`)
- `--domain` flag in `playbooks create` now accepts forward slashes for nested paths (e.g., `--domain agents/operations`)
- Automatic cleanup of empty parent directories when deleting playbooks
- New `path_helpers.rs` module with shared ID/path conversion utilities
- Handle orphaned Docker volumes and containers in `cloud tenant create`
- "External PostgreSQL" option for local tenants to use custom database connection strings

### Changed
- Playbook scanning now uses recursive directory traversal (unlimited depth)
- Reduce scheduler job log verbosity from `info` to `debug` level
- Hide `profile create` from interactive menu (still available via direct CLI for power users)
- Credential errors are now fatal except for `FileNotFound` (which allows local-only mode)
- `cloud status` now displays resolved credentials path using typed paths
- Local tenant database names now include unique suffix to prevent conflicts across projects (e.g., `local_19c22e8f38b`)
- Profile bin path now dynamically resolves to debug or release based on which binary is newer
- Local tenant creation now prompts for database source: Docker or External PostgreSQL

### Fixed
- Add process existence check before sending SIGTERM in MCP cleanup
- Credentials path resolution now uses `ProjectContext` typed paths instead of profile-relative strings
- Sync token warning no longer shown for local tenants (only applies to cloud tenants)
- Profile validation no longer fails when only debug build exists
- Migration errors now propagate with actual output instead of silent failure
- Admin user sync only runs after successful migrations
- Remove hardcoded content sources from `cloud init` templates (now generates empty config)
- Docker container reuse now retrieves password directly from container (no more password prompts across projects)
- Replace `unwrap_or_default()` with explicit `map_or_else` patterns per Rust standards

## [0.1.0] - 2026-02-02

### Added
- `systemprompt admin provider` subcommand for AI provider management
- List, show, enable/disable AI providers from CLI

### Fixed
- Resolve clippy errors and warnings

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-27

### Changed
- Update `playbooks show` to take args by reference for efficiency
- Use char array pattern in `title_case` function

### Fixed
- Fix clippy pedantic warnings across crate

## [0.0.16] - 2026-01-27

### Added
- Add `systemprompt core playbooks create` command to create new playbooks with category/domain structure
- Add `systemprompt core playbooks edit` command to modify playbook frontmatter and instructions via `--set KEY=VALUE`
- Add `systemprompt core playbooks delete` command with confirmation prompt and `--all` support
- Add `systemprompt core content edit` command to modify content fields (title, description, body, keywords, etc.)
- Support `--public/--private` flags for content visibility in edit command

## [0.0.15] - 2026-01-26

### Added
- Add `systemprompt core playbooks show <id>` command to display full playbook content
- Add `--raw` flag to output playbook content without formatting

## [0.0.14] - 2026-01-26

### Changed
- Prominently feature playbooks in CLI help output with "GETTING STARTED" section
- Update main CLI description to reference playbooks for workflow guides
- Update core command description to list playbooks first

## [0.0.13] - 2026-01-26

### Added
- Add `systemprompt playbooks list` command to list playbooks from disk with category filtering
- Add `systemprompt playbooks list <id>` to show playbook details
- Add `systemprompt playbooks sync` command for bidirectional disk/database synchronization
- Support playbook directory structure: `services/playbook/{category}/{domain}.md`

## [0.0.12] - 2026-01-26

### Added
- Display active profile banner on each command showing profile name, target type (local/cloud), and tenant ID

### Fixed
- `--profile` flag now accepts direct file paths (e.g., `--profile ./my-profile.yaml` or `--profile /absolute/path/to/profile.yaml`)

## [0.0.11] - 2026-01-26

### Added
- `systemprompt core artifacts list` command to list artifacts with optional context filtering
- `systemprompt core artifacts show` command to inspect artifact details and content
- `--schema` flag for `systemprompt plugins mcp tools` to display parameter schemas in readable format

### Changed
- Centralize path resolution with `ResolvedPaths` struct
- Extract `ExecutionEnvironment` for deployment detection
- Unify `CommandDescriptor` trait replacing `HasRequirements`
- Deduplicate session creation with shared helpers
- Flatten `run()` pipeline and extract CLI args module
- Standardize non-interactive mode with centralized `interactive` module
- Add `confirm_optional()` for optional confirmations with defaults
- Migrate all confirmation patterns to `require_confirmation()` or `confirm_optional()`

### Fixed
- Eliminate legacy session system, unify on `SessionStore`
- `--profile` flag now properly overrides active session
- Use `as_str()` instead of `to_string()` for session token in routing
- Allow `JWT_SECRET` and `DATABASE_URL` to sync during deploy
- Resolve session management bugs and improve CLI session UX
- Fix clippy errors across workspace

## [0.0.10] - 2026-01-25

### Fixed
- CLI session authentication bugs
- Clippy nested or-patterns and formatting

## [0.0.9] - 2026-01-25

### Added
- Auto-create admin user for cloud profiles if user doesn't exist in database

### Changed
- `admin session switch` now always outputs confirmation message regardless of interactive mode
- `infra services start` no longer requires cloud credentials for local profiles

### Fixed
- Updated all error messages to reference `admin session login` instead of deleted `infra system login`
- Cloud profiles now try secrets file first when running locally, even if `source: env`
- Cloud profiles now use session token from `sessions/index.json` instead of requiring separate cloud login
- Local profiles no longer fail with 401 when cloud credentials are expired or missing
- Session switch now properly displays session key for debugging

## [0.0.8] - 2026-01-25

### Changed
- `admin session login` now supports cloud profiles with direct database access
- Session key derived from profile's `tenant_id` instead of hardcoded local

### Removed
- Removed `infra system login` command (use `admin session login` instead)
- Removed `infra system` subcommand group

### Fixed
- Cloud profile guard now allows `Admin::Session` commands

## [0.0.7] - 2026-01-23

### Changed
- `cloud tenant rotate-credentials` now displays both Internal URL and External URL on separate lines

## [0.0.6] - 2026-01-23

### Fixed
- Fix `admin session login` failing with "No profile loaded" even when SYSTEMPROMPT_PROFILE is set
- Session login now properly initializes profile and secrets from environment variable or --profile flag

## [0.0.5] - 2026-01-23

### Changed
- Update inter-crate dependency versions

## [0.0.4] - 2026-01-23

### Added
- `systemprompt cloud tenant cancel` command to cancel cloud subscriptions
- Automatic profile creation during local tenant setup (matching cloud tenant behavior)
- Cloud tenant sync on `tenant list` command - fetches tenants from API
- Paddle customer portal link in tenant list header
- `systemprompt infra db migrations status` command to show migration status for all extensions
- `systemprompt infra db migrations history <extension>` command to show migration history

### Changed
- Profile builders now include `extensions` configuration field
- Local tenant creation now prompts for profile name and API keys

### Fixed
- Fix tenant sync overwriting credentials with masked URLs from API
- Add warning when saving secrets with masked database URLs
- Fix schema validation for VIEW-based schemas
- Add migration system infrastructure

## [0.0.2] - 2026-01-22

### Changed
- Implement distributed schema registration pattern
- Each domain crate now owns its SQL schemas via Extension trait
- Remove centralized module loaders from systemprompt-loader

### Fixed
- Fix `include_str!` paths that pointed outside crate directory
- Ensure crate compiles standalone when downloaded from crates.io

## [0.0.1] - 2026-01-21

- Initial release
