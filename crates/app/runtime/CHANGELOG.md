# Changelog

## [0.9.2] - 2026-05-14

### Added
- `RuntimeError` and `RuntimeResult` for typed error handling across the runtime surface.
- `context_loaders` module exposing `load_geoip_database` and `load_content_config` helpers.
- `context_traits` module for context-facing trait surfaces.
- `with_marketplace_filter` on `AppContextBuilder` for marketplace ACL injection.

### Changed
- Move `AppContextBuilder` into its own `builder.rs` module.
- Split context resource loading out of `context.rs` into `context_loaders.rs`.

### Removed
- `installation` module and its `install_module` / `install_module_with_db` entry points; install flows now live in `systemprompt-database`.

## [0.1.21] - 2026-04-01

### Changed
- Move `AppContext` construction logic from `new_internal` into `AppContextBuilder::build`.
- Add `AppContextParts` to keep `AppContext::from_parts` within the argument-count limit.
- Initialize logging immediately after database pool creation so subsequent tracing events are persisted.
- Remove the redundant `init_logging` call from `serve.rs`.

## [0.1.18] - 2026-03-27

### Changed
- Upgrade to Rust 2024 edition.

## [0.1.10] - 2026-02-08

### Added
- `AppContext::content_routing` accessor returning `Option<Arc<dyn ContentRouting>>`.
- `RouteClassifier` integration with content routing for URL classification.

## [0.1.0] - 2026-02-02

### Changed
- First stable release at the unified workspace version.

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency.

## [0.0.4] - 2026-01-23

### Added
- Export `FilesConfigValidator` from the startup validation module.

### Fixed
- Schema validation now handles VIEW-based schemas correctly.
- Wire in the migration system infrastructure.

## [0.0.2] - 2026-01-22

### Changed
- Each domain crate now owns its SQL schemas via the `Extension` trait.

### Removed
- Centralized module loaders from `systemprompt-loader`.

### Fixed
- Correct `include_str!` paths that pointed outside the crate directory.
- Ensure the crate compiles standalone when consumed from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
