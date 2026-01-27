# Changelog

## [0.0.13] - 2026-01-27

### Added
- UI renderer module in MCP crate with template-based HTML generation for artifacts
- Renderers for Dashboard, Chart, Table, Form, List, Image, and Text artifact types

### Changed
- Update all workspace crate dependencies to 0.0.13
- Refactor inline CSS/JS to separate asset files in MCP crate
- Fix clippy pedantic warnings across workspace

## [0.0.11] - 2026-01-26

### Changed
- Update all workspace crate dependencies to 0.0.11
- Improve CLI session management and path resolution
- Add engagement fan-out for analytics events
- Fix clippy errors across workspace

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
