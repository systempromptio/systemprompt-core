# Changelog

## [0.38.0] - 2026-08-25

### Changed

- The BSD/GNU `ps` divergence lives in `services/process/ps.rs`; `monitor` and `pid` delegate to it rather than each shelling out with their own flags.

## [0.36.0] - 2026-08-23

### Breaking

- **Breaking:** `McpDomainError` gains a `PortHolderUnverifiable { port, pid, service }` variant. Migrate by adding an arm to any exhaustive `match` over the enum.

### Added

- `classify_port_holder` distinguishes a port holder whose identity cannot be established from one established as foreign, and `cleanup_port_processes` reports the two separately. The old message blamed "another systemprompt installation or an unrelated service" for what was, on a platform without an identity check, simply unknown.

### Fixed

- Port reclamation works on macOS. Every holder used to classify as foreign there, so a `SIGKILL`ed server's own orphaned MCP child kept its port and the next start failed with `PortOwnedByForeignProcess` instead of reclaiming it.
- `get_process_info` returned `None` for every pid on macOS: it asked `ps` for the GNU-only `cmd` output keyword, which BSD `ps` rejects outright, so the command failed and the result was read as "no such process". It now asks for `command`, accepted by both.
- `find_process_on_port_with_name` never matched on macOS, silently skipping the by-name port cleanup the orchestrator's rebuild path depends on. `ps -o comm=` yields a bare command name under GNU `ps` but a full executable path under BSD `ps`; the value is now normalised to the file name before it is compared with the configured server name.

## [0.33.0] - 2026-08-20

### Added

- `build_tool_list_result` and `build_resource_template_list_result` helpers construct list results with the protocol-appropriate SEP-2549 cache metadata applied.

### Fixed

- `tools/list` results were missing the SEP-2549 `ttlMs`/`cacheScope` cache metadata required by protocol `2026-07-28`; tool and resource-template list results are now stamped like the resource list/read results.

## [0.32.0] - 2026-08-18

### Breaking

- **Breaking:** built against `rmcp` 3.1.3; re-exported `rmcp` types follow. Migrate by building against the same `rmcp` minor.
- **Breaking:** `execute_tool_call` takes an optional `SharedElicitationDelegate` as its final argument. Migrate by passing `None`, or use `McpClient::call_tool`, which is unchanged.

### Added

- Protocol `2026-07-28` support with backward compatibility: `mcp_protocol_version` is pinned to `2026-07-28`, `mcp_supported_protocol_versions` exposes the full negotiable set, and `create_router` serves legacy sessions and stateless `2026-07-28` requests from the same service.
- `ElicitationDelegate` / `SharedElicitationDelegate` route server elicitation requests (form and URL modes) to a human; the elicitation capability is only advertised when a delegate is installed, and requests received without one are declined.
- The `io.modelcontextprotocol/tasks` extension is declared in `build_extension_capabilities` and the outbound client capabilities; task handles from `tools/call` are polled to completion, and `TaskManager`, `TaskContext`, and `TaskOptions` are re-exported for server binaries.
- `Mcp-Method`/`Mcp-Name` operation headers are logged on inbound requests and forwarded across `create_proxy_router`.
- `tools/list` pagination cursors are followed to exhaustion.
- Resource list/read results carry SEP-2549 `ttlMs` and `cacheScope`.

### Fixed

- `create_proxy_router` streams response bodies and preserves upstream response headers instead of buffering the body and dropping every header; hop-by-hop headers are stripped and oversized request bodies are rejected with `413`.

## [0.30.1] - 2026-08-07

### Fixed

- `resolve_artifact_type` reads the canonical `x-artifact-type` tag before the `artifact_type` serde envelope tag when falling through a `cli` envelope, matching the agent-side type inference.

## [0.30.0] - 2026-08-07

### Breaking

- **Breaking:** `McpToolExecutor::execute` and `McpResponseBuilder::new` take a `&ClientProfile` describing the negotiated client. Migrate by building one inside `call_tool` with `client_profile_from_peer(&context)`, or from persisted `initialize` params with `client_profile_from_stored`.
- **Breaking:** tool results are shaped per client. The embedded `ui://` resource and `io.systemprompt/ui-resource-uri` are sent only to hosts that negotiated the `io.modelcontextprotocol/ui` extension; `structuredContent` only to clients on protocol `2025-06-18` or later; any other client — including one whose `initialize` declaration is unknown — receives text content only, with the artifact body folded into the text block.
- **Breaking:** `McpResponseBuilder::new` takes a `ToolIdentity` naming the server and the tool instead of a bare tool name. Migrate by passing `ToolIdentity::new(server_name, tool_name)`. The executor previously passed the tool name where the artifact path expected the server, so `ui://` resource URIs were minted as `ui://<tool-name>/…` and every widget resolution failed; migration `002_artifact_server_name_repair` rewrites the affected `mcp_artifacts.server_name` rows from the execution record.
- **Breaking:** `structuredContent` and the advertised `outputSchema` carry the tool's typed output directly instead of the `ToolResponse` envelope, and execution provenance (including `artifact_id` and `mcp_execution_id`) moves to `_meta["io.systemprompt/execution"]`. Bare snake_case `_meta` keys are gone — MCP reserves unprefixed `_meta` keys. Migrate consumers by reading the payload from `structuredContent` and identifiers from the meta key.

### Added

- `McpOutputSchema::text_body` names the plain-text body of an output; text-bearing artifacts provide it so text-only clients receive the data, and other outputs fall back to pretty-printed JSON under the summary.
- `McpToolHandler::read_only` (default `false`) advertises the MCP `readOnlyHint` annotation, and `McpToolHandler::tool_definition` builds the canonical `tools/list` entry — name, description, both schemas, the annotation, and the UI meta — so servers stop hand-rolling `Tool` values that drift from the wire contract.

### Fixed

- Artifact `ui://` resource URIs name the MCP server that minted them instead of the tool, so hosts that validate the URI authority against the connector name can resolve the resource. A migration repairs previously persisted `mcp_artifacts` rows from the execution record.
- `McpOutputSchema::validated_schema` guarantees an **object schema**: `"type": "object"` is inserted at the root when schemars omits it. The MCP spec requires a tool's `outputSchema` to be an object schema, and Claude Desktop enforces it strictly — a tagged-enum output (`CliArtifact`) previously advertised a bare `oneOf` with no `type`, and the client parked the entire server at connect time on the first such tool. Sound for every implementor: tagged-enum variants all serialize as objects.

## [0.29.0] - 2026-08-05

### Changed

- The RBAC middleware sources the marketplace attribute floor from `systemprompt_security::authz::member_attribute_floor`; the dependency on `systemprompt-marketplace` is removed.

## [0.28.0] - 2026-07-31

### Added

- The outbound MCP client declares the Enterprise-Managed Authorization extension in its `ClientCapabilities`, which tells a server it may answer an unauthenticated call with an EMA challenge instead of driving us into an interactive authorization redirect no user is present to complete.
- `AuthChallenge` parses a `WWW-Authenticate` challenge and `ProtectedResourceMetadata` describes what the resource behind it expects, so a 401 from an MCP server now says whether an ID-JAG is wanted and which authorization servers issue it, rather than echoing the raw header. The metadata URL comes from the peer, so it is dialled only through the shared outbound guard.

## [0.27.0] - 2026-07-29

### Breaking

- **Breaking:** `McpConnectionResult.tools_count`, `HealthCheckDetails.tools_available`, `ServiceStatus.tools_count`, and `McpServiceStatus.tools_count` are `Option<usize>`. `None` means the tool list was never enumerated — an OAuth-gated server is probed for reachability only — and `Some(0)` means a server was asked and answered with none. Migrate by matching on the option at each render site.

### Added

- `mcp_session_cleanup` reads its retention window from the `retention_days` job parameter (default 7, the previous constant).

### Fixed

- Startup no longer reports `tools=0` for an OAuth-gated MCP server. The fabricated zero read as "this server exposes no tools" on the startup log, `ServiceStatus`, and the `McpServerReady` event alike. The startup line now omits the `tools` field when the count is unmeasured, carries `validation_type`, and reads "MCP service validated; tool list not enumerated".

- `validate_connection` reports a name and version for a peer that sends no `Implementation`, which a server answering `server/discover` (SEP-2575) is permitted to do. Both fall back to the configured service name and `1.0.0`.

### Changed

- The MCP SDK moves from `rmcp` `3.0.0-beta.1` to the released `3.0.0`. The enabled feature set is unchanged.

## [0.26.0] - 2026-07-28

### Breaking

- **Breaking:** the dashboard and chart renderers consume the typed artifact models via the same typed-payload path as card/message/media. The dashboard renderer previously parsed a loose dialect (`type`, top-level `id`/`width`, flat `metrics`/`items` arrays) that no producer emitted — the typed models serialize `section_type`, `section_id`, `layout`, and nested `data` structs, so every typed dashboard rendered with empty section bodies. The loose dialect is deleted; a section whose `data` does not match its declared `section_type` is a render error rather than an empty body.
- **Breaking:** charts are inline SVG rendered by the new `chart_svg` module rather than Chart.js. Neither renderer loads a script from `cdn.jsdelivr.net` any more, so both emit a plain strict CSP in place of one exempting that host, and `assets/js/chart.js` is deleted. A rendered artifact now draws identically with no network. Anything asserting on the old `<canvas>`, on `window.CHART_CONFIG`, or on the CDN in a CSP header needs updating.

### Added

- `ArtifactTheme` and `register_artifact_theme!` let a deployment restyle rendered artifact UI without forking a renderer. Every stylesheet addresses colour, radius, shadow, and type through the `--mcpui-*` custom properties in `tokens.css`; a registered theme re-declares some or all of them and inherits the rest, with `extra_css` for what a custom property cannot express. Registration is compile-time, matching the existing scanner and route-selector registries.

### Fixed

- Typed `DashboardSection` payloads render their bodies, ids, layout widths, and ordering; `Timeline` and `Text` sections render (a timeline section previously fell through to empty text).
- Chart titles, chart types, and axis labels survive storage. The chart renderer read them exclusively from `metadata.rendering_hints`, which the stored-artifact path never populated — every stored chart rendered as an untitled bar chart.

## [0.25.0] - 2026-07-27

### Fixed

- MCP server subprocesses are spawned through `subprocess::spawn_supervised`, so the kernel `SIGTERM`s them if the supervisor dies rather than leaving them holding their ports.
- The event bus no longer logs `error=channel closed` when publishing an event with no broadcast subscribers. `broadcast::Sender::send` returns that error precisely when the receiver count is zero; nothing was closed and nothing was lost, since the handler fan-out is a separate list that still runs.
- `DeploymentService::validate_config` no longer validates the merged configuration a second time; `ConfigLoader::load` has already validated what it returns.

## [0.24.0] - 2026-07-26

### Breaking

- **Breaking:** the MCP SDK moves to rmcp `3.0.0-beta.1`, which changes two `ServerHandler` return types every downstream server implementation overrides. Migrate by returning the response enum: `call_tool` yields `CallToolResponse` and `read_resource` yields `ReadResourceResponse`, both reachable from the old result type with `.into()`.
- **Breaking:** rmcp's paginated result types (`ListToolsResult`, `ListResourcesResult`, ...) gained `result_type`, `ttl_ms`, and `cache_scope` fields, so struct literals no longer compile. Migrate by constructing them with `T::with_all_items(items)`.
- **Breaking:** `tools/call` responses now carry a `"resultType": "complete"` discriminator on the wire (SEP-2663). Results serialized before this release still deserialize, defaulting to `complete`, but strict JSON consumers must tolerate the new field.

### Added

- `MAX_REQUEST_BODY_BYTES` (4 MiB) is applied to the MCP router explicitly, so oversized POST bodies get a `413` at a limit this crate owns rather than one inherited from the SDK.
- The streamable-HTTP client enforces rmcp's `max_sse_event_size` budget. Previously the setting was accepted and silently ignored, because the SDK can only apply it inside a client implementation and ours parses SSE itself.

### Changed

- `HttpClientWithContext::get_stream` takes `session_id: Option<Arc<str>>`, matching rmcp 3.0's stateless-resume path; `None` omits the session header and resumes from `last_event_id` alone.

## [0.23.0] - 2026-07-24

### Breaking

- **Breaking:** the `cli` module and its `start_services`, `stop_services`, `show_status`, and `list_services` display helpers are removed; the CLI drives `McpOrchestrator` directly. Migrate by calling the orchestrator methods and rendering the result with your own output sink.

### Changed

- The UI renderer emits card, list, and table content as structured JSON values rather than pre-formatted strings, so `--json` output carries real nested JSON.

## [0.22.0] - 2026-07-21

### Breaking

- **Breaking:** `CspPolicy` gains a `media_src` field, emitted as the `media-src` directive. Migrate by constructing policies through `CspBuilder` rather than a struct literal.

### Added

- Tool results always carry the `io.systemprompt/ui-resource-uri` `_meta` key (`UI_RESOURCE_URI_META_KEY`) naming the artifact's `ui://` resource, so a host that does not forward embedded resource content blocks can reach the rendered artifact through `resources/read`.
- `artifact_resource_uri` and `parse_artifact_resource_uri` build and parse `ui://` artifact resource URIs; `artifact_shell_template` exposes the artifact shell markup.
- Tool results embed the server-rendered artifact as a `text/html;profile=mcp-app` resource block, and `read_artifact_resource` serves the same markup from `resources/read`.
- `PresentationCardRenderer`, `MessageRenderer`, `AudioRenderer`, `VideoRenderer`, and `CopyPasteTextRenderer` complete `create_default_registry` coverage of every `CliArtifact` variant.

### Changed

- `tool_ui_meta` builds `_meta.ui` from the typed `McpUiToolMeta` and also writes the legacy `ui/resourceUri` key for hosts that predate it.
- Rendered artifact documents carry the generated `MCP_UI` method constants and report their `{width, height}` to the host through `ui/notifications/size-changed`.
- `TableRenderer` reads rows from `items`, the field `TableArtifact` serializes, alongside `data` and `rows`.

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
