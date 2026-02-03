# Changelog

## [0.1.1] - 2026-02-03

### Changed
- Support nested playbook directory structures in diff calculator and sync
- Use recursive WalkDir scanning (no depth limit) for playbook discovery
- Export playbooks to nested directories based on domain path separators
- Clean up empty parent directories when deleting orphan playbooks

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-26

### Added
- Add `PlaybooksDiffCalculator` for comparing disk and database playbooks
- Add `PlaybooksLocalSync` with bidirectional sync support (disk ↔ database)
- Add `export_playbook_to_disk()` and `generate_playbook_markdown()` functions
- Add playbook diff models: `DiskPlaybook`, `PlaybookDiffItem`, `PlaybooksDiffResult`
- Support playbook directory structure: `services/playbook/{category}/{domain}.md`

## [0.0.11] - 2026-01-26

### Changed
- Update content sync to use simplified ingestion API without content type filtering

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
