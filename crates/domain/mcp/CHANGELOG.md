# Changelog

## [0.22.0] - 2026-07-21

### Added

- Tool results always carry the `io.systemprompt/ui-resource-uri` `_meta` key (`UI_RESOURCE_URI_META_KEY`) naming the artifact's `ui://` resource, so a host that does not forward embedded resource content blocks can reach the rendered artifact through `resources/read`.
- `artifact_resource_uri` and `parse_artifact_resource_uri` build and parse `ui://` artifact resource URIs; `artifact_shell_template` exposes the artifact shell markup.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Breaking

- rmcp is upgraded to 2.x (MCP 2025-11-25 specification alignment). Public surfaces that carried `Content`/`RawContent` now use `rmcp::model::ContentBlock`; resource metadata is built through the `Resource` builder and resource sizes are `u64`. The JSON wire format is unchanged.
- The minimum supported Rust version is 1.94.

## [0.18.0] - 2026-07-01

### Added

- `McpClient::resolve_external_proxy_target` returns the provider URL and per-user outbound headers for an external MCP server, letting an HTTP gateway forward to the provider with a server-side-minted bearer while withholding the systemprompt credential.

### Changed

- Accessor-backed external MCP servers (those declaring `external_auth`) are reported healthy without an unauthenticated provider probe. The monitor holds no per-user token to authenticate with, so the previous probe reported such servers as spuriously unhealthy.

## [0.17.0] - 2026-06-24

### Changed

- The streamable-HTTP MCP client runs on the workspace `reqwest` (0.12) through rmcp 1.8's transport trait, supplying its own context-propagating HTTP client instead of rmcp's bundled reqwest-backed transport. This removes a duplicate `reqwest` 0.13 from the dependency tree.

## [0.16.1] - 2026-06-22

### Fixed

- External MCP servers are no longer started as local subprocesses. Enabling an external server alongside internal ones previously aborted startup when the orchestrator tried to spawn the external server as a process (resolving an empty binary path); external servers now have no lifecycle footprint and are reached only at their configured remote endpoint.

## [0.16.0] - 2026-06-22

### Breaking

- **Breaking:** The `McpServerRegistry` type alias is removed. Migrate by using `RegistryService`.
- Error enum tuple variants that wrapped a bare message string are now struct variants with a named `message` field; match arms and constructors change from `Error::Foo(msg)` to `Error::Foo { message: msg }`.
- The minimum supported Rust version is 1.88.

### Added

- External MCP servers resolve a per-user third-party bearer from an extension-served accessor (`external_auth.token_endpoint`) and inject it on the configured header, replacing the systemprompt credential so nothing internal reaches the third party. Static `headers` configured on the server are also forwarded.

### Changed

- Over-long functions were split into focused helpers to satisfy the workspace's 75-line function ceiling. No behavioural or API change.

### Fixed

- External MCP servers are now reached at their configured remote endpoint for tool calls instead of the internally-derived gateway URL.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.13.1] - 2026-06-01

### Fixed

- MCP server cleanup never signals the calling process when tearing down a managed child server.

## [0.13.0] - 2026-05-28

### Changed

- `mcp::Deployment.endpoint` is now `Option<String>`. For `internal` servers it must be a relative path (e.g. `/api/v1/mcp/<name>/mcp`) or omitted; absolute URLs are rejected at config-load time. The gateway derives the public URL from `server.api_external_url + endpoint`. `external` MCP servers continue to accept absolute upstream URLs.
- `Deployment.mcp_servers` and related catalog lists adopt `PluginComponentRef { source, include, exclude }` for uniformity with the rest of the services config; flat-list YAML is rejected.
- `port_probe::is_port_in_use` is now bound by a 1 s connect timeout; previously a stuck SYN could hang the bootstrap probe indefinitely.
- `BridgeHookError::HookTokenRejected { status, body }` is now a typed variant on the bridge hook path, replacing the prior stringly-typed wrapping; lets the API gateway surface the upstream rejection body to clients.

## [0.12.0] - 2026-05-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.11.0] - 2026-05-20

### Changed
- Workspace-aligned release. MCP server lifecycle and transport surfaces unchanged.

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
