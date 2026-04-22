# Changelog

## [0.3.0] - 2026-04-22

### Changed
- `AiQuotaBucketRepository::increment` now takes `IncrementParams` struct (groups `tenant_id`, `user_id`, `window_seconds`, `window_start`, `delta`)
- `AiRequestPayloadRepository::upsert_request` and `upsert_response` now take `UpsertPayloadParams` struct (groups `body`, `excerpt`, `truncated`, `bytes`)
- New public types: `IncrementParams`, `UpsertPayloadParams` exported from crate root

## [0.1.3] - 2026-03-20

### Added
- `OpenAiStreamChunk`, `OpenAiStreamChoice`, `OpenAiStreamDelta` typed structs for OpenAI streaming
- Pricing-based cost calculation in `StreamStorageWrapper` using `ModelPricing`
- Token usage tracking (input, output, total, cache read, cache creation) accumulated during streaming

### Changed
- All provider streaming implementations return `StreamChunk` instead of raw strings
- `StreamStorageWrapper` captures token usage and finish reason from `StreamChunk::Usage` during streaming
- Replace `serde_json::Value` with typed `OpenAiStreamChunk` struct in OpenAI streaming parser
- `capture_usage` accepts `StreamChunk` directly instead of individual parameters

## [0.1.2] - 2026-02-03

### Added
- `StreamStorageWrapper` for capturing and storing streaming AI response data
- Request storage tracking for `generate_stream` and `generate_with_tools_stream` methods

### Changed
- `RequestStorage` is now `Clone` to support stream wrapper ownership
- **BREAKING**: Cost tracking changed from `cost_cents` (INTEGER) to `cost_microdollars` (BIGINT) for sub-cent precision
- Regenerated SQLx offline query cache

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
