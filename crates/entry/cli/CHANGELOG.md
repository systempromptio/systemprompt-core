# Changelog

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
