# Changelog

## [0.16.0] - 2026-06-10

### Breaking

- The minimum supported Rust version is 1.88.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.1] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.0] - 2026-05-28

### Changed

- Workspace version bump; no API changes in this crate.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Removed
- Unused `async-trait` and `indexmap` dependencies.

## [0.9.2] - 2026-05-14

### Added
- `EmbeddedDefaultsProvider` bundling the in-tree `defaults/` templates so consumers get a usable engine without filesystem access.
- `json` Handlebars helper for emitting values inside JSON contexts such as JSON-LD `<script>` blocks.
- `RegistryStats` reporter exposing template counts and source breakdowns.

### Changed
- Split `registry` into `lifecycle`, `queries`, and `stats` submodules.
- Re-exported provider traits (`ComponentRenderer`, `PageDataProvider`, `TemplateDataExtender`, `EmbeddedLoader`, `FileSystemLoader`) from `systemprompt-template-provider`.

## [0.1.18] - 2026-03-27

### Changed
- Upgraded to Rust 2024 edition.
- Improved embedded defaults initialization.

## [0.1.1] - 2026-02-03

### Changed
- Sorted components by priority on registration in `TemplateRegistry`.
- Added priority field to component registration debug logging.

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone aligning all crates to 0.1.0.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.3] - 2026-01-22

### Fixed
- Fixed schema validation for VIEW-based schemas.
- Added migration system infrastructure.

## [0.0.2] - 2026-01-22

### Changed
- Implemented distributed schema registration so each domain crate owns its SQL schemas via the `Extension` trait.
- Removed centralized module loaders from `systemprompt-loader`.

### Fixed
- Fixed `include_str!` paths that pointed outside the crate directory.
- Ensured the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
