# Changelog

## [0.1.12] - 2026-02-10

### Changed
- Remove `/vite.svg` special case from `RouteClassifier` static asset detection

## [0.1.10] - 2026-02-08

### Added
- `resolve_slug()` method on `ContentRouting` trait (with default `None` impl)
- `ContentRouting` implementation for `ContentConfigRaw`
- `extract_slug_from_pattern` helper for URL pattern slug extraction
- `ContentRouting` blanket impl for `Arc<T>` where `T: ContentRouting`

### Changed
- `RouteClassifier` now accepts optional `ContentRouting` provider

## [0.1.9] - 2026-02-05

### Added
- `MarkdownFrontmatter` struct for YAML frontmatter in markdown responses
- `MarkdownResponse` struct for content with frontmatter and body
- `ContentNegotiationConfig` struct for server content negotiation settings
- Builder methods for `MarkdownFrontmatter` (description, author, published_at, tags, url)

### Changed
- `ServerConfig` now includes `content_negotiation` configuration field

## [0.1.4] - 2026-02-04

### Added
- `capabilities` module with MCP UI extension types
- `McpExtensionId` enum for extension identification
- `McpAppsUiConfig` struct for MCP Apps UI configuration
- `ToolVisibility` enum with `Model` and `App` variants
- `McpCspDomains` struct with builder pattern for CSP domain configuration
- `McpResourceUiMeta` struct for resource UI metadata
- `JwtAudience::Resource(String)` variant for RFC 8707 resource indicator support
- `WWW-Authenticate` header with `resource_metadata` on all 401 responses (MCP OAuth 2.1 compliance)

### Changed
- Refactor `Secrets::get()` to use `char::is_uppercase` method reference
- Remove doc comments from `ToolUiConfig` methods per standards
- `JwtAudience` is no longer `Copy` (now contains `Resource(String)` variant)
- `JwtClaims::has_audience()` now takes `&JwtAudience` instead of `JwtAudience`

## [0.1.3] - 2026-02-03

### Added
- `ActivityRequest` and `ActivityData` types for cloud activity tracking
- `ApiPaths::CLOUD_ACTIVITY` endpoint constant
- `ApiPaths::ACTIVITY_EVENT_LOGIN` and `ApiPaths::ACTIVITY_EVENT_LOGOUT` event type constants

### Removed
- `WebhooksConfig` and `UserEventsWebhookConfig` from profile configuration
- `webhooks` field from `Profile` struct

## [0.1.2] - 2026-02-03

### Added
- `AiResponse::with_streaming()` builder method to mark responses as streaming

## [0.1.1] - 2026-02-03

### Changed
- Replace `unwrap_or_default()` with explicit `map_or_else` patterns in secrets and profile loading

### Removed
- Remove `credentials_path` and `tenants_path` fields from `CloudConfig` (use typed paths via `ProjectContext`)
- Remove `Profile::credentials_path()` and `Profile::tenants_path()` methods

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.14] - 2026-01-27

### Added
- `ToolUiConfig` struct for configuring UI metadata in MCP tool definitions
- `ToolUiConfig::to_meta_json()` method for generating UI metadata JSON

## [0.0.13] - 2026-01-27

### Changed
- Use `Self::` instead of type name in Part enum match arms for clippy compliance

## [0.0.11] - 2026-01-26

### Changed
- `ToolResponse::to_json()` now returns `Result<JsonValue, serde_json::Error>` instead of silently returning `Null` on error
- `Artifact::to_json_value()` trait method now returns `Result<JsonValue, serde_json::Error>` instead of silently returning `Null` on error

## [0.0.7] - 2026-01-23

### Changed
- `RotateCredentialsResponse` now returns `internal_database_url` and `external_database_url` instead of single `database_url` field

## [0.0.4] - 2026-01-23

### Added
- `tenant_subscription_cancel` API path for subscription cancellation
- `ExtensionsConfig` struct for profile-based extension enable/disable configuration
- `extensions` field in `Profile` struct
- `is_masked_database_url` helper to detect masked credentials

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
