# Changelog

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.21.0] - 2026-07-16

### Breaking

- `serve::execute_with_events` takes a `ServeOptions` struct (`foreground`, `kill_port_process`, `run_migrations`) in place of discrete flags.

### Fixed

- `infra services start` installed extension schemas twice — once during its own bootstrap and again when the API-serve phase built its context; the serve phase now skips the redundant migration run.

## [0.20.0] - 2026-07-15

### Fixed

- Every CLI invocation with a stale persisted session minted a new `CLI Session - {profile}` row in `user_contexts` with no GC path. The three CLI session paths now reuse one stable row per user/profile via `ContextRepository::get_or_create_cli_context`.
- `infra logs trace show <task-id> --json` emitted nothing for AI-task traces; it now emits the serialized trace view as the Trace JSON artifact, matching the log-event-trace path.

## [0.19.0] - 2026-07-02

### Breaking

- The minimum supported Rust version is 1.94.
- SQLx is upgraded to 0.9.
- rmcp is upgraded to 2.x.

### Changed

- Session, content-source, and tenant identifiers are typed through the command surface, and interactive cloud/web/MCP/admin flows are driven through a `Prompter` seam. No user-facing command behaviour changes.

## [0.17.1] - 2026-06-30

### Changed

- `admin session switch` is metadata-only: it rewrites the active-profile pointer without booting the profile being switched away from, so it works even when the active profile is a cloud target with an expired session. It no longer logs in as a side effect — run `admin session login` to authenticate the target.
- `infra jobs run` confirms before running against a remote/cloud profile; pass `--yes` to bypass the prompt.

### Fixed

- `admin session switch` exits non-zero on failure, and the error shown when a command cannot run against an external/cloud database names the active profile and the remedy.

## [0.17.0] - 2026-06-24

### Changed

- MCP client commands (`plugins mcp`, `admin agents`) connect through the shared reqwest-0.12 streamable-HTTP client rather than rmcp's bundled reqwest-backed transport (rmcp 1.8), removing a duplicate `reqwest` 0.13 from the dependency tree.

## [0.16.1] - 2026-06-22

### Changed

- Generated profiles populate the new EMA fields (`id_jag_ttl_secs`, `TrustedIssuer` allowlists) with their defaults.

## [0.16.0] - 2026-06-22

### Breaking

- The minimum supported Rust version is 1.88.

### Changed

- Over-long functions were split into focused helpers to satisfy the workspace's 75-line function ceiling. No behavioural or API change.

## [0.14.0] - 2026-06-01

### Changed

- `admin setup` and `admin config` read and write the new provider registry: the setup wizard, catalog, and profile builders produce profiles with a `providers:` section, and provider selection threads through the merged configuration.

## [0.13.1] - 2026-06-01

### Added

- `admin config` subcommands edit a profile's `gateway`, `governance`, `security`, `secret`, and `catalog` sections in place, validating the result before writing it back.
- `admin setup` accepts `--default-provider` (and offers interactive selection) to designate the gateway default provider, plus a `--force` flag that overwrites existing profile, catalog, and secrets files.

### Changed

- `admin setup` generates a complete, bootable profile — gateway catalog (providers, models, routes), governance and authz sections, and the gateway-required `hook` resource audience — rather than an empty shell.

## [0.13.0] - 2026-05-28

### Changed

- `admin profile show` formats the rendered profile through the new typed `PluginComponentRef` projections so marketplace `mcp_servers`, `skills`, `agents`, and `plugins` lists display as the unified `{ source, include, exclude }` object rather than the legacy flat sequence.
- Bootstrap DDL paths (`commands/admin/setup/**`) widened to write the per-tenant `governance.authz` block expected by 0.13.0 routers when provisioning a fresh database.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Added
- Sync and cloud-deploy commands aligned with the Service-JWT handshake; `admin config validate` exposes the new `JsonSchema`-driven profile validation surface.
- `infra db migrate-repair --apply` subcommand reconciles checksum drift in place (see `systemprompt-database` 0.11.0).

### Changed
- Workspace-aligned release; CLI prose and per-item rustdoc trimmed under the 0.10.x publishing pass.

## [0.10.2] - 2026-05-16

### Changed

- `bootstrap::init_credentials_gracefully` now recovers from any local-mode-recoverable cloud credential error (`CloudError::is_local_mode_recoverable`), broadening the earlier fresh-clone-only handling to also cover expired or invalid credentials.

## [0.9.2] - 2026-05-12

### Fixed

- `bootstrap::init_credentials_gracefully` now pattern-matches `CloudError::CredentialsFileNotFound` directly, restoring fresh-clone bootstrap on local profiles without a credentials file.

### Removed

- Drop unused `init_credentials()` helper.

## [0.4.3] - 2026-04-29

### Added

- `systemprompt admin cowork rotate-signing-key` generates a fresh ed25519 seed, persists it to the secrets file, and prints the base64 public key.

## [0.3.0] - 2026-04-22

### Changed

- Format `admin cowork` commands and regenerate the SQLx offline query cache.

## [0.2.4] - 2026-04-20

### Fixed

- `admin agents registry` now defaults to the active profile's `api_external_url`, honours `--url` as an override, and falls back to `localhost:8080` only when no profile is loaded.

## [0.2.3] - 2026-04-20

### Fixed

- Local-trial profiles no longer require cloud credentials; `create_new_session` routes `SessionKey::Tenant("local_*")` profiles to the local-session path and `resolve_local_user_email` falls back to `admin@localhost.dev` when no hint is provided.

### Changed

- `bootstrap.rs` `is_local_profile` predicate now delegates to `Profile::is_local_trial()`.

## [0.2.0] - 2026-04-15

### Changed

- `cloud profile show` uses `ConfigLoader::load()` in place of `EnhancedConfigLoader::from_env()?.load()`.

## [0.1.24] - 2026-04-14

### Fixed

- Local-only profiles (`cloud.tenant_id` prefixed `local_`, or `cloud.validation: warn`/`skip`) no longer surface a `Cloud credential error: Credentials not initialized` line on startup; the message is downgraded to a `debug!` log.

## [0.1.23] - 2026-04-14

### Fixed

- `admin agents message` and `admin agents task` now send A2A v1.0.0 method names (`SendMessage`, `SendStreamingMessage`, `GetTask`); previous calls were rejected by the server with `Unsupported method`.

## [0.1.21] - 2026-04-02

### Changed

- `admin session login` now connects to the database before checking cached sessions, enabling DB validation.

### Fixed

- Validate cached CLI sessions against the database before reuse; stale sessions are detected and removed.
- Use `ApiPaths` constant for the default agent endpoint in `admin agents create`.

## [0.1.18] - 2026-03-27

### Added

- `cloud tenant` gains `cancel`, `show`, `list`, `edit`, and `delete` subscription commands.

### Changed

- Upgrade to Rust 2024 edition.
- Split large CLI modules into focused files across `tenant`, `secrets`, `services`, and `config` commands.

## [0.1.17a] - 2026-02-26

### Fixed

- Rename `total_cents` to `total_cost_microdollars` in analytics overview to reflect the underlying unit.
- Rename `avg_cost_per_request_cents` to `avg_cost_per_request_microdollars` in the cost summary output.
- `format_cost()` now divides by 1,000,000 (microdollars) instead of 100 (cents).

## [0.1.17] - 2026-02-19

### Added

- `core agents list` command with `--enabled` / `--disabled` filters.
- `core agents show <name>` command for agent details.
- `core agents sync` command for bidirectional disk-database sync.
- `core agents validate [name]` command for configuration validation.

## [0.1.16] - 2026-02-19

### Changed

- Hooks CLI now uses `HookEvent::ALL_VARIANTS` and `matchers_for_event()` rather than hardcoded event-name strings.
- `count_hooks` in `plugin show` iterates the `HookEvent` enum instead of chaining fields manually.

## [0.1.15] - 2026-02-18

### Changed

- Consolidate duplicate `SkillConfig`, `ParsedSkill`, `strip_frontmatter`, and `parse_skill_from_config` into shared types.
- Replace `unwrap_or("")` with explicit error handling in skills list and plugin agent generation.
- Emit `tracing::warn!` for silently skipped YAML parse errors in agent generation.
- Replace magic `"config.yaml"` and `"index.md"` string literals with shared constants.

## [0.1.14] - 2026-02-17

### Added

- `core plugins list` command with `--enabled` / `--disabled` filters.
- `core plugins show <id>` command for plugin details.
- `core plugins validate [id]` command for configuration validation.
- `core plugins generate [--id <id>]` command for marketplace artefact generation.
- `core hooks list` command to list hooks across plugins.
- `core hooks validate` command for hook configuration validation.
- Marketplace JSON generation in `plugins generate` for Claude Code plugin distribution.

### Changed

- Split `plugins/generate.rs` into six focused modules under `generate/`.
- Replace magic string comparisons with `ComponentSource` / `ComponentFilter` enum matching.
- Extract `DEFAULT_AGENT_TOOLS` and `PLUGIN_ROOT_VAR` constants.
- Emit `tracing::warn!` on silent error paths in marketplace and hook scanning.
- Introduce `PluginGenerateContext` to reduce function parameter count.

### Removed

- **Breaking:** `systemprompt core playbooks` subcommand group (`create`, `edit`, `delete`, `list`, `show`, `sync`). Migrate by using `core skills` or marketplace plugins for prompt distribution.

## [0.1.13] - 2026-02-11

### Fixed

- Skip external database URL routing when running on Fly.io; the container must use the internal URL.

## [0.1.12] - 2026-02-11

### Added

- Auto-generate sync tokens during `cloud deploy` when none is configured, saving the token to profile secrets.
- `external_database_url` field in generated cloud profile secrets.
- Cloud profiles with `external_db_access` route CLI commands to the external database URL.

### Changed

- Refactor session login into a reusable `login_for_profile()` helper for profile-specific authentication.
- `admin session switch` now loads secrets directly from the target profile instead of relying on global bootstrap.
- `cloud db` commands use `Secrets::load_from_path()` and `effective_database_url()` rather than manual JSON parsing.

## [0.1.11] - 2026-02-11

### Fixed

- `cloud db` commands now prefer `external_database_url` from `secrets.json`, falling back to `database_url`.
- `--database-url` global flag is now accepted by `cloud db` subcommands.

## [0.1.10] - 2026-02-10

### Fixed

- `cloud tenant create` now waits for backend provisioning to complete via SSE with polling fallback before fetching secrets, fixing a race where credentials were unavailable immediately after checkout.

### Changed

- Version bump for workspace consistency with analytics and content routing changes.

## [0.1.4] - 2026-02-07

### Fixed

- Ensure the `systemprompt_admin` PostgreSQL role exists during local tenant creation; the role is now verified and created via the `postgres` superuser before any database operations.

## [0.1.3] - 2026-02-03

### Added

- Cloud activity tracking for CLI login and logout events via `POST /api/v1/activity`.
- `ApiPaths::ACTIVITY_EVENT_LOGIN` and `ApiPaths::ACTIVITY_EVENT_LOGOUT` constants.

### Changed

- `cloud auth logout` is now async to support activity reporting.

## [0.1.2] - 2026-02-03

### Added

- Initialise logging with the database pool in the `admin agents run` command.

### Changed

- Analytics cost displays use `cost_microdollars` for sub-cent precision.
- Regenerate the SQLx offline query cache.

## [0.1.1] - 2026-02-03

### Added

- Support nested playbook directory structures (e.g. `domain/agents/operations.md`); playbook IDs map underscores to path separators.
- `--domain` flag in `playbooks create` accepts forward slashes for nested paths.
- Auto-load user email from credentials for `admin session login`; `--email` is no longer required.
- Handle orphaned Docker volumes and containers in `cloud tenant create`.
- "External PostgreSQL" option for local tenants to use custom database connection strings.
- New `path_helpers.rs` module with shared ID/path conversion utilities.
- Automatic cleanup of empty parent directories when deleting playbooks.

### Changed

- Playbook scanning uses recursive directory traversal at unlimited depth.
- Reduce scheduler job log verbosity from `info` to `debug`.
- Hide `profile create` from the interactive menu; the command remains available directly.
- Credential errors are now fatal except for `FileNotFound`, which allows local-only mode.
- `cloud status` displays the resolved credentials path using typed paths.
- Local tenant database names include a unique suffix to prevent conflicts across projects.
- Profile bin path resolves dynamically to debug or release based on the newer binary.
- Local tenant creation prompts for database source (Docker or external PostgreSQL).

### Fixed

- Add process existence check before sending `SIGTERM` in MCP cleanup.
- Credentials path resolution uses `ProjectContext` typed paths instead of profile-relative strings.
- Suppress the sync-token warning for local tenants; it only applies to cloud tenants.
- Profile validation no longer fails when only a debug build exists.
