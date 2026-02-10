# Changelog

## [0.1.12] - 2026-02-10

### Added
- ETag support with `If-None-Match` → `304 Not Modified` on all static file responses
- `Cache-Control: no-cache` headers on HTML page responses (previously missing entirely)
- `Cache-Control: public, max-age=3600` headers on metadata files (sitemap, robots, feed)

### Changed
- Rename `vite.rs` → `static_files.rs` (no Vite dependency exists)
- Replace blocking `std::fs::read()` with `tokio::fs::read()` in all static file handlers
- Refactor `serve_static_content` into focused functions under 75-line limit
- Remove dead `serve_html_with_analytics` function (analytics handled by middleware + client JS)

## [0.1.11] - 2026-02-08

### Added
- Content routing integration for analytics and engagement routes
- Automatic content ID resolution from URL slugs in analytics events
- Content routing passed through `AnalyticsState` and `EngagementState`

### Fixed
- `record_events_batch` now correctly passes content routing to `resolve_content_id`

## [0.1.10] - 2026-02-06

### Added
- Site-wide authentication gate middleware (`site_auth_gate`)
- Extensions can now declare site auth requirements via `site_auth()` trait method
- Unauthenticated static content requests redirect to configured login path
- Static assets (CSS, JS, images, fonts) bypass the auth gate
- Extension-declared public prefixes bypass the auth gate

## [0.1.9] - 2026-02-05

### Added
- Content negotiation middleware with `Accept` header parsing
- `AcceptedFormat` extractor for determining requested response format
- `AcceptedMediaType` enum supporting JSON and Markdown formats
- Support for `.md` URL suffix as alternate markdown format request
- HTTP `Link` header with alternate format URLs in content responses
- Markdown response format for blog/content endpoints

### Changed
- Content handlers now use `AppContext` instead of direct `DbPool` injection
- Blog content endpoint returns markdown when `Accept: text/markdown` or `.md` suffix used

## [0.1.4] - 2026-02-04

### Added
- RFC 8707 `resource` parameter support in authorize endpoint
- RFC 8707 `resource` parameter support in token endpoint
- Resource URI validation (must be valid HTTPS/HTTP URI without fragment)
- `TokenGenerationParams.resource` field for resource-scoped tokens

### Changed
- `AuthorizeQuery` and `AuthorizeRequest` now include `resource` field
- `TokenRequest` now includes `resource` field
- `WebAuthnCompleteQuery` now includes `resource` field
- WebAuthn form template context now includes `resource` parameter

## [0.1.3] - 2026-02-03

### Changed
- Simplified `create_oauth_state()` - removed webhook publisher configuration (now uses cloud activity API)

## [0.1.2] - 2026-02-03

### Changed
- Regenerated SQLx offline query cache

## [0.1.1] - 2026-02-03

### Fixed
- Session middleware now gracefully handles JWT tokens referencing non-existent users by creating new anonymous session instead of error spam

## [0.1.0] - 2026-02-02

### Changed
- First stable release milestone
- All crates now at consistent 0.1.0 version

## [0.0.13] - 2026-01-27

### Changed
- Use `expect()` instead of `unwrap()` in artifact response builder for better error messages

## [0.0.11] - 2026-01-26

### Changed
- Rename `AnalyticsState` fields to remove redundant `_repo` postfix
- Improve session middleware handling

### Added
- Fan out engagement metrics from `PageExit` events in analytics routes
- Batch analytics event processing with engagement fan-out
- Session validation in `JwtContextExtractor` to auto-create missing sessions for OAuth tokens issued before session persistence fix

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
