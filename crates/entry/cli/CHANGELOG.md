# Changelog

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
