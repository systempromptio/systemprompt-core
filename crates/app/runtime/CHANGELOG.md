# Changelog

## [0.1.21] - 2026-04-01

### Changed
- Move `AppContext` construction logic from `new_internal()` into `AppContextBuilder::build()` — builder owns its construction
- Extract `AppContextBuilder` into `builder.rs` to keep `context.rs` under 300-line limit
- Add `AppContextParts` struct to avoid too-many-arguments in `from_parts()`
- Move `init_logging()` call earlier — immediately after DB pool creation, capturing all subsequent tracing events in DB
- Remove redundant `init_logging()` call from `serve.rs`

## [0.1.18] - 2026-03-27

### Changed
- Upgrade to Rust 2024 edition

## [0.1.10] - 2026-02-08

### Added
- `content_routing()` accessor on `AppContext` returning `Option<Arc<dyn ContentRouting>>`
- `RouteClassifier` integration with content routing for URL classification

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency

## [0.0.4] - 2026-01-23

### Added
- Export `FilesConfigValidator` from startup validation module

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
