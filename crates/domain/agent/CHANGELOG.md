# Changelog

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-26

### Added
- Add Playbook domain type with `Playbook` model, `PlaybookRow`, and `PlaybookMetadata`
- Add `PlaybookRepository` with full CRUD operations and category filtering
- Add `PlaybookIngestionService` for parsing markdown files with YAML frontmatter
- Add `PlaybookService` for loading playbooks and listing metadata
- Add `agent_playbooks` database schema with category/domain organization
- Register playbook schema in `AgentExtension`

## [0.0.12] - 2026-01-26

### Fixed
- Auto-create replacement context when MCP tool call has invalid/stale context_id instead of failing with "Context validation failed"

## [0.0.11] - 2026-01-26

### Changed
- Pass JWT provider to OAuth validation in A2A request handler
- Use short-form type imports in `task_helper.rs`

### Fixed
- Filter null JSON values in `ArtifactBuilder::build_artifacts()` to prevent errors when tools return `Some(Value::Null)`

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
