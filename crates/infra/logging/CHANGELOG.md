# Changelog

## [0.1.2] - 2026-02-03

### Changed
- Updated trace queries to use `cost_microdollars` for cost tracking
- Regenerated SQLx offline query cache

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency

## [0.0.11] - 2026-01-26

### Added
- Include error message in MCP execution trace events for failed tool calls
- `CliService::profile_banner()` method for displaying active profile information to stderr

### Changed
- Improve CLI service output and prompts handling

## [0.0.3] - 2026-01-22

### Changed
- Logging extension marked as required (`is_required() -> true`)

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
