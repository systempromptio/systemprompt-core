# Changelog

## [0.11.0] - 2026-05-20

### Changed
- Workspace-aligned release. Loader surface unchanged.

## [0.9.2] - 2026-05-14

### Added
- `expose-internals` Cargo feature gating test-only entry points such as `ConfigLoader::load_from_content` for use by dependent crates outside `cfg(test)`.

### Changed
- Split `config_loader` and `extension_loader` into submodules (`includes`, `merge`, `types`, `manifest`, `result`) for clearer separation between parsing, merging, and result types.

## [0.2.0] - 2026-04-15

### Breaking
- **Breaking:** `ConfigLoader` no longer auto-appends discovered agent files to the root `config.yaml` `includes:` list; migrate by listing every include explicitly in `config.yaml`.
- **Breaking:** Removed `EnhancedConfigLoader`, `IncludeResolver`, `ConfigLoader::discover_and_load_agents`, and `ConfigWriter::add_include`; migrate by using `ConfigLoader` directly with explicit `includes:` entries.

### Added
- Recursive `includes:` resolution with cycle detection.

### Changed
- Consolidated config loading into a single `ConfigLoader` with static-method shims preserving the prior public API.

## [0.1.18] - 2026-03-27

### Changed
- Upgraded to the Rust 2024 edition.

## [0.1.1] - 2026-02-03

### Added
- `ExtensionLoader::resolve_bin_directory()` utility that picks `target/debug` or `target/release` based on binary modification time.

### Fixed
- Resolved a clippy `unnested_or_patterns` warning in `resolve_bin_directory`.

## [0.1.0] - 2026-02-02

### Changed
- First stable release at the unified workspace version.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.11] - 2026-01-26

### Removed
- Removed the standalone secrets loader; secrets are now loaded through the config system.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure.

### Fixed
- Schema validation for VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Adopted a distributed schema registration pattern in which each domain crate owns its SQL schemas via the `Extension` trait.

### Removed
- Centralised module loaders previously hosted in this crate.

### Fixed
- Corrected `include_str!` paths that pointed outside the crate directory so the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
