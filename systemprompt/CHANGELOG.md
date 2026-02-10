# Changelog

## [Unreleased]

### Added
- Read/write database pool separation for multi-region Postgres deployments
- `DATABASE_WRITE_URL` support in secrets (env var and secrets.json)
- `Database::from_config_with_write()` constructor for dual-pool setup
- `Database::write_pool_arc()`, `write_pool()`, `write_provider()`, `has_write_pool()` methods
- `ConfigProvider::database_write_url()` trait method with backward-compatible default
- `DatabaseContext::from_urls()` for explicit read/write URL configuration

### Changed
- All 37 repositories migrated to use write pool for mutations (INSERT/UPDATE/DELETE)
- Migrations and schema installations now use write pool to prevent failures on read replicas
- `Database::begin()` now uses write pool for transactions
- `Database::test_connection()` validates both read and write pools

### Fixed
- Migrations failing with "cannot execute CREATE FUNCTION in a read-only transaction" when DATABASE_URL points to a read replica

## [0.1.9] - 2026-02-05

### Added
- Content negotiation middleware with `Accept` header parsing for format selection
- Support for markdown responses via `text/markdown` Accept header or `.md` URL suffix
- `MarkdownFrontmatter` and `MarkdownResponse` types for structured markdown output
- `ContentNegotiationConfig` in server profile for configuring negotiation behavior
- `AcceptedFormat` and `AcceptedMediaType` types for content type handling
- HTTP `Link` header with alternate format URLs when content negotiation is enabled

### Fixed
- Fix `MigrationService` to ensure `extension_migrations` table exists before querying
  - Prevents "relation does not exist" errors on fresh database initialization
  - Adds defensive `ensure_migrations_table_exists()` check using `CREATE TABLE IF NOT EXISTS`

### Changed
- Content handlers now use `AppContext` instead of direct `DbPool` injection
- Refactor engagement analytics repository for improved type safety

## [0.1.4] - 2026-02-04

### Added
- MCP artifacts persistence with `mcp_artifacts` table for storing tool execution results
- `McpArtifactRepository` with CRUD operations for artifact storage and retrieval
- MCP Apps UI extension support (`io.modelcontextprotocol/ui`) for rich tool responses
- `ToolVisibility` enum for controlling tool visibility (model/app)
- `McpCspDomains` builder for Content Security Policy domain configuration
- `StreamStorageParams` struct to reduce function parameter count
- `TraceSummaries` struct for grouping trace summary parameters

### Changed
- Refactor secrets loading to use method references instead of closures
- Move CSP tests from source file to dedicated test crate
- Improve code quality across workspace for clippy pedantic compliance
- Refactor `DatabaseSessionManager` to take `&DbPool` reference
- CLI commands refactored to fix underscore-prefixed binding warnings
- Replace `map().unwrap_or()` with idiomatic `map_or()` patterns
- Replace `map().unwrap_or_else()` with `map_or_else()` on Result types
- Use `let...else` syntax instead of match for single-pattern destructuring
- Remove redundant clones in early return paths

### Fixed
- Fix redundant closure warnings in secrets and capabilities modules
- Fix unused self parameter in CSP policy extraction
- Fix branch code duplication in OAuth challenge builder
- Fix `TraceEvent` missing `latency_ms` field error in trace output builder
- Fix type mismatches in CLI command execute functions

## [0.1.2] - 2026-02-03

### Added
- Streaming response storage tracking via `StreamStorageWrapper`
- `AiResponse::with_streaming()` builder method
- Logging initialization in agent run command

### Changed
- `RequestStorage` now implements `Clone`

## [0.1.1] - 2026-02-03

### Added
- Support nested playbook directory structures (`domain/agents/operations.md`)
- Playbook IDs map underscores to path separators (`domain_agents_operations` → `domain/agents/operations.md`)
- Handle orphaned Docker volumes and containers in cloud tenant creation
- Docker container reuse now retrieves password directly from container environment
- Priority-based deduplication for page prerenderers (higher-priority prerenderer wins per page type)
- Priority-based deduplication for component renderers (higher-priority component wins per variable)
- Components now sorted by priority on registration in `TemplateRegistry`

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
