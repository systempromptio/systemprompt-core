# Changelog

## [0.1.1] - 2026-02-03

### Added
- Support nested playbook directory structures (`domain/agents/operations.md`)
- Playbook IDs map underscores to path separators (`domain_agents_operations` → `domain/agents/operations.md`)
- Handle orphaned Docker volumes and containers in cloud tenant creation
- Docker container reuse now retrieves password directly from container environment

### Changed
- Reduce scheduler job log verbosity (info → debug)
- Credential errors are now fatal except for `FileNotFound` (allows local-only mode)
- Cloud paths use `ProjectContext` typed paths instead of profile-relative strings
- Update recommended PostgreSQL version to 18-alpine in README
- Replace `unwrap_or_default()` with explicit `map_or_else` patterns per Rust standards

### Fixed
- Add process existence check before SIGTERM in MCP cleanup
- Clear invalid JWT cookies when user no longer exists instead of repeated FK constraint errors
- Fix clippy unnested or-patterns warning in extension loader

## [0.1.0] - 2026-02-02

### Added
- Anthropic web search support via `web_search_20250305` tool
- OpenAI web search support
- CLI provider subcommand for AI provider management
- Updated AI provider models with latest versions

### Changed
- First stable release milestone
- All 30 crates now at consistent 0.1.0 version

## [0.0.14] - 2026-01-27

### Added
- Early branding config validation on startup (copyright, twitter_handle, logo, favicon, display_sitename)
- `BrandingConfigRaw` struct for structured web.yaml branding validation
- Content templates documentation (`instructions/information/content-templates.md`)
- `EmbeddedDefaultsProvider` with homepage template fallback

### Changed
- Empty/missing image fields now default to placeholder instead of failing
- Improved error messages show available templates when template not found

### Fixed
- `image: ""` in frontmatter no longer causes publish to fail

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
