# Changelog

## [0.1.0] - 2026-02-02

### Added
- Anthropic web search support via `web_search_20250305` tool
- OpenAI web search support
- Updated AI provider models with latest versions

### Fixed
- Use correct model configs for image providers and search capabilities

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-27

### Changed
- Version bump for workspace consistency

## [0.1.0] - 2026-01-26

### Fixed
- Fix Gemini Google Search grounding API error "Function calling config is set without function_declarations" by removing `tool_config` from search requests (only needed for function calling, not for Google Search grounding)

## [0.0.11] - 2026-01-26

### Fixed
- Force Gemini to use Google Search grounding by setting `tool_config` with `mode: Any` instead of relying on AUTO mode

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
