# Changelog

## [0.10.2] - 2026-05-15

### Added
- Resilience layer around MCP tool calls: a per-attempt timeout, retry with
  exponential backoff, a per-server circuit breaker, and a concurrency limit,
  configured via the `mcp.resilience` block. Health-check failures feed the same breaker.
- `McpDomainError::Timeout`, `CircuitOpen`, and `DependencyUnavailable` variants,
  plus `McpDomainError::classify`.

### Changed
- **Breaking:** `McpToolProvider::new` now takes a `&ResilienceSettings` argument.
- **Breaking:** the `mcp` config block replaces the flat `connect_timeout_ms`,
  `execution_timeout_ms`, and `retry_attempts` keys with a nested `resilience` block.
- MCP tool-call RPCs are now bounded by an execution timeout; previously only
  connection setup was bounded.

## [0.9.2] - 2026-05-14

### Changed

- Normalised changelog format to match workspace standard.

## [0.4.3] - 2026-04-29

### Fixed

- Propagate `MANIFEST_SIGNING_SECRET_SEED` into spawned subprocess environments so manifest signing seeds remain stable across launches.

## [0.1.18] - 2026-03-27

### Added

- MCP request logging middleware capturing method, URI, session ID, and auth status.
- Proxy-verified identity auth flow in RBAC middleware.
- Stale session cleanup in `DatabaseSessionManager`.

### Changed

- Upgraded to Rust 2024 edition.
- Split the dashboard UI renderer into focused modules.

### Fixed

- MCP session loss no longer produces 404 on SSE reconnect; sessions persist to the database.
- Moved MCP session auth to the proxy layer with trusted identity headers.

## [0.1.6] - 2026-03-05

### Added

- `artifact_type()` and `artifact_type_name()` methods on the `McpOutputSchema` trait.
- `McpOutputSchema` implementations for Audio, Chart, Cli, CopyPasteText, Dashboard, Image, List, PresentationCard, Table, Text, and Video artifacts.

### Changed

- **Breaking:** Renamed `call_tool` to `McpToolExecutor`. Migrate by updating imports and type references.
- **Breaking:** Merged `build()` and `build_and_persist()` into a single `build()` that always persists artifacts. Migrate by removing calls to `build_and_persist()` and using `build()`.

## [0.1.5] - 2026-02-19

### Changed

- Populated `server_type` and `remote_endpoint` fields in MCP server config from deployment.
- Replaced inline validation with dedicated `RegistryValidator` methods for target resolution.
- Refactored the orchestrator to route server targets via a dedicated `TargetResolution` module.

## [0.1.4] - 2026-02-04

### Added

- `mcp_artifacts` table schema for persisting MCP tool execution artifacts.
- `McpArtifactRepository` with save, find, list, delete, and cleanup operations.
- `CreateMcpArtifact` and `McpArtifactRecord` data structs.
- `capabilities` module exposing MCP Apps UI extension helpers.
- `mcp_apps_ui_extension()` for experimental capabilities.
- `tool_ui_meta()` helper for UI metadata generation.

### Changed

- **Breaking:** `DatabaseSessionManager::new()` now takes `&DbPool` instead of an owned value. Migrate by passing a reference.
- **Breaking:** Renamed `UiMetadata::for_artifact()` to `for_static_template()`. Migrate by updating call sites.
- `UiMetadata::with_prefers_border()` is now `const fn`.
- UI metadata uses the static `/artifact-viewer` template path instead of per-artifact ID substitution.

### Removed

- **Breaking:** Removed `result_ui_meta()` helper. Migrate to static templates via `ui/notifications/tool-result`.
- **Breaking:** Removed `UiMetadata::to_result_meta()`. Migrate by relying on static templates.
- **Breaking:** Removed `ARTIFACT_ID_PLACEHOLDER` from `ui_renderer`. Migrate by using static template paths.

### Fixed

- Stale sessions are cleaned up and `SessionNeedsReconnect` is returned when the SSE channel closes mid-resume.
- Removed redundant `artifact_id.clone()` in `McpResponseBuilder::build()`.
- Replaced a redundant closure in UI metadata CSP conversion.
- Replaced `map().flatten()` with `and_then()` in the response builder.

## [0.1.2] - 2026-02-03

### Added

- `mcp_sessions` table for persisting MCP session state across server restarts.
- `McpSessionRepository` with CRUD operations for session persistence.
- `DatabaseSessionManagerError` enum with specific session error variants.

### Changed

- `DatabaseSessionManager` now uses a hybrid in-memory plus database persistence model.
- Registered the `mcp_sessions` schema in `McpExtension`.

### Fixed

- Eliminated the infinite token refresh loop after server restart by persisting MCP sessions to the database.
- `DatabaseSessionManager` now uses the `DbPool` parameter previously ignored.
- Session resumption returns `SessionNeedsReconnect` when the session exists in the database but not in memory.

## [0.1.1] - 2026-02-03

### Changed

- Replaced `unwrap_or_default()` with explicit `map_or_else` patterns in UI renderer templates.

### Fixed

- Cleanup now checks process existence before sending `SIGTERM` to avoid errors on already-terminated processes.

## [0.1.0] - 2026-02-02

### Changed

- First stable release aligning all workspace crates at version 0.1.0.

## [0.0.14] - 2026-01-27

### Added

- `UiMetadata::for_tool_definition()` factory for tool-specific UI metadata.
- `UiMetadata::to_tool_meta()` for generating tool metadata JSON.
- `UiMetadata::to_result_meta()` for generating result metadata with artifact ID substitution.

### Changed

- Added an `include` directive to `Cargo.toml` to support SQLx offline mode in published crates.

## [0.0.13] - 2026-01-27

### Added

- UI renderer module providing template-based HTML generation for artifacts.
- Renderers for Dashboard, Chart, Table, Form, List, Image, and Text artifacts.
- Asset loading via `include_str!` for CSS and JS files.
- CSP builder with configurable directives.

### Changed

- Moved inline CSS and JS to separate asset files.
- Brought the crate into clippy pedantic compliance.

## [0.0.3] - 2026-01-22

### Added

- Migration system infrastructure.

### Fixed

- Schema validation now handles VIEW-based schemas.

## [0.0.2] - 2026-01-22

### Changed

- **Breaking:** Each domain crate now owns its SQL schemas via the `Extension` trait. Migrate by removing references to centralised loaders and registering schemas through `Extension`.
- Removed centralised module loaders from `systemprompt-loader`.

### Fixed

- Corrected `include_str!` paths that pointed outside the crate directory.
- Crate now compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

### Added

- Initial release.
