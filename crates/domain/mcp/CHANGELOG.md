# Changelog

## [0.0.14] - 2026-01-27

### Added
- `UiMetadata::for_tool_definition()` factory method for creating tool-specific UI metadata
- `UiMetadata::to_tool_meta()` method for generating tool metadata JSON
- `UiMetadata::to_result_meta()` method for generating result metadata with artifact ID substitution

### Changed
- Add `include` directive to Cargo.toml for SQLx offline mode support in published crates

## [0.0.13] - 2026-01-27

### Added
- UI renderer module with template-based HTML generation for artifacts
- Renderers for Dashboard, Chart, Table, Form, List, Image, and Text artifact types
- Asset loading via `include_str!` for CSS and JS files
- CSP (Content Security Policy) builder with configurable directives

### Changed
- Refactor inline CSS/JS to separate asset files for maintainability
- Update code for clippy pedantic compliance

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
