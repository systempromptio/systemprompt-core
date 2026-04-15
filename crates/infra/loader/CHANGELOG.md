# Changelog

## [0.2.0] - 2026-04-15

### Added
- Recursive `includes:` resolution with cycle detection.

### Changed
- `ConfigLoader` is now the single loader; the public API is preserved via static-method shims.

### Removed
- `EnhancedConfigLoader`, `IncludeResolver`, `ConfigLoader::discover_and_load_agents`, and `ConfigWriter::add_include`.

### Breaking
- Loading is now pure — the loader no longer auto-adds discovered agent files to the root `config.yaml` `includes:` list. Users must list every include explicitly.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade to Rust 2024 edition

## [0.1.1] - 2026-02-03

### Added
- `ExtensionLoader::resolve_bin_directory()` utility to dynamically resolve target/debug or target/release based on binary modification time

### Fixed
- Fix clippy unnested or-patterns warning in `resolve_bin_directory`

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency

## [0.0.11] - 2026-01-26

### Removed
- Remove standalone secrets loader (now integrated into config system)

## [0.0.3] - 2026-01-22

### Fixed
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
