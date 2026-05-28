# Changelog

## [0.12.2] - 2026-05-28

### Changed

- `mcp::Deployment.endpoint` is now `Option<String>`. The struct gains a `validate(name)` method that rejects absolute URLs for `internal` servers; `ServicesConfig::validate` invokes it for every entry in `mcp_servers`. `external` servers continue to accept absolute upstream URLs.
- `AgentCardConfig::skills` is now `#[serde(default, skip_serializing)]` and deprecated. The A2A `card.skills` view is computed at serve time by joining `agent.metadata.skills` against the on-disk `services/skills/` catalog; authored `card.skills:` arrays in agent YAML are tolerated for one release (so downstream repos can land their YAML cleanup separately) but are ignored. `AgentConfigValidator` no longer requires `card.skills[].id` to resolve on disk — only `metadata.skills` ids are validated. See root CHANGELOG.

### Added

- `profile::GATEWAY_REQUIRED_RESOURCE_AUDIENCES` (currently `["hook"]`) names the audience strings the gateway's grant paths hard-require to appear in `security.allowed_resource_audiences`. `Profile::validate` now rejects bootstrap with a one-line error per missing entry, so deployments whose profiles haven't opted into the internal `hook` audience fail at startup instead of returning 400 `invalid_target` on the first bridge `client_credentials` hook-scope request.

## [0.12.0] - 2026-05-27

### Breaking

- `JwtClaims.department` and `AuthzRequest.department` removed; replaced by `attributes: BTreeMap<String, serde_json::Value>`. Token issuers populate the bag with namespaced keys (`acme.desk`, `boeing.clearance`); extension hooks read `req.attributes.get("your.key")`.
- `JwtUserContext.department` removed; `attributes: BTreeMap<String, serde_json::Value>` added so the gateway path forwards them onto every `AuthzRequest`. `JwtUserContext.roles: Vec<String>` narrowed to a single `role: Permission`.
- `SessionParams.department: Option<String>` replaced by `attributes: BTreeMap<String, serde_json::Value>`.
- `AuthzContext` enum replaced with `{ kind: Cow<'static, str>, payload: serde_json::Value }`. Core mints three kinds — `"none"`, `"gateway.invocation"` (`{ "model": ... }`), `"mcp.tool_call"` (`{ "tool": ... }`) — via `AuthzContext::none()` / `gateway_invocation(&ModelId)` / `mcp_tool_call(&McpToolName)`. Tenants extend via `AuthzContext::extension(kind, payload)`. Typed accessors `gateway_invocation_model()` / `mcp_tool_call_tool()` return `None` on kind mismatch.
- `AccessControlConfig.departments` and `RuleEntry.departments` removed; the exported `DepartmentEntry` type is gone. YAML files with top-level `departments:` or per-rule `departments:` arrays are rejected by `deny_unknown_fields`. `IngestReport.departments_declared` removed.
- `Profile.gateway` is now `Option<GatewayState>` (enum `Spec(GatewayConfigSpec) | Resolved(GatewayConfig)`); runtime read paths call `GatewayState::resolved() -> Option<&GatewayConfig>`. The on-disk `gateway.catalog_path: <path>` field is removed — write `gateway.catalog: { path: "..." }` for the file-backed form or `gateway.catalog: { providers: [...], models: [...] }` for the inline form. The runtime `GatewayConfig` loses `Deserialize` / `schemars::JsonSchema` and is constructed only via `GatewayConfigSpec::resolve(profile_dir)`.
- `ServicesConfig.content` field removed; `services/content/config.yaml` is loaded directly. The `pub mod content` declaration is gone; the loader aggregator no longer wraps the file under a `content:` key.

### Added

- `AuthzContext::{NONE_KIND, GATEWAY_INVOCATION_KIND, MCP_TOOL_CALL_KIND}` const literals and `AuthzContext::extension(kind, payload)` constructor for tenants minting their own enforcement-site kinds.
- `GatewayConfigSpec`, `GatewayCatalogSource`, `GatewayState` public types exported from `systemprompt_models::profile`, mirroring the existing `GatewayPolicySpec` / `GatewayPolicyConfig` pattern in the AI domain.

## [0.11.0] - 2026-05-20

### Breaking
- `JwtAudience::Cowork` renamed to `JwtAudience::Bridge`; `as_str()` now returns `"bridge"`. Migrate by re-issuing JWTs minted under the old name; tokens with the previous audience no longer validate.

### Added
- `JsonSchema` derives across the profile config tree (`profile/{security,governance,runtime,gateway,server,cloud,site,paths,...}`) so profiles can be introspected and validated against a generated schema.
- `auth::enums` adjustments to align audiences with the Service-JWT sync handshake.

## [0.4.3] - 2026-04-29

### Added
- `JwtAudience::Cowork` variant on `auth::enums`, covered by `as_str` and `FromStr`.
- `SecretsBootstrap::manifest_signing_secret_seed` accessor returning `Result<[u8; 32], _>`.

### Fixed
- `Secrets::parse` strips JSON `null` values from the root object before deserialization, so literal `"openai": null` and `"gemini": null` no longer fail with `invalid type: null, expected a string`.

## [0.2.3] - 2026-04-20

### Added
- `CloudConfig::is_local_trial` and `Profile::is_local_trial` predicate identifying local-trial profiles (no `cloud` block, `tenant_id` starts with `local_`, or `validation` is `Warn`/`Skip`).

## [0.2.0] - 2026-04-15

### Breaking
- **Breaking:** `ServicesConfig.web` is now `Option<WebConfig>` using the full `systemprompt_provider_contracts::WebConfig` type. Migrate by replacing `WebConfig { branding: BrandingConfig { site_name, logo_url, primary_color } }` constructors with the full provider-contracts `WebConfig`.
- **Breaking:** `ServicesConfig` and `PartialServicesConfig` now use `#[serde(deny_unknown_fields)]`. Migrate by removing any unknown keys from services configuration.

### Added
- `ContentConfig` wrapper at `services::content::ContentConfig`.
- `SkillsConfig` as a first-class field on `ServicesConfig`.
- `content_sources` binding field on `PluginConfig`.
- `ServicesConfig::validate` enforces plugin bindings (agents, mcp_servers) and skill map-key integrity.

### Removed
- `services::web` stub module.
- `FullWebConfig` and `WebBrandingConfig` aliases from the crate root.

### Fixed
- Removed 15 forbidden `///` doc comments from `macros.rs` per project coding standards.

## [0.1.23] - 2026-04-14

### Added
- `a2a::methods` module exposing A2A v1.0.0 JSON-RPC method name constants (`SendMessage`, `SendStreamingMessage`, `GetTask`, `CancelTask`, `SubscribeToTask`, `GetExtendedAgentCard`, and the four push notification config methods).

## [0.1.21] - 2026-04-02

### Added
- `ApiPaths::SYNC_BASE` and `ApiPaths::ANALYTICS_BASE` constants.
- `CloudEnterpriseLicenseInfo` struct for domain-based enterprise licensing.
- Optional `enterprise` field on `UserMeResponse` with `#[serde(default)]`.
- `EnterpriseLicenseInfo` type alias.

### Changed
- `ServiceCategory::base_path` and `ServiceCategory::matches_path` use `ApiPaths` constants instead of hardcoded strings.

## [0.1.20] - 2026-03-20

### Breaking
- **Breaking:** `AiProvider` trait streaming methods now return `StreamChunk` instead of `String`. Migrate by matching on `StreamChunk::Text` / `StreamChunk::Usage` at call sites.

### Added
- `StreamChunk` enum with `Text` and `Usage` variants for typed streaming responses.
- `cache_read_tokens`, `cache_creation_tokens`, and `finish_reason` fields on `StreamChunk::Usage`.

## [0.1.19] - 2026-03-05

### Changed
- CLI artifact moved from `cli.rs` to a `cli/` module directory with `mod.rs` and `conversion.rs`.
- All artifact types (`Audio`, `Card`, `Chart`, `Cli`, `CopyPasteText`, `Dashboard`, `Image`, `List`, `Table`, `Text`, `Video`) expose an `ARTIFACT_TYPE_STR` constant, and `ArtifactType::Display` uses them in place of hardcoded strings.

## [0.1.18] - 2026-02-19

### Added
- `DiskAgentConfig` struct for on-disk `services/agents/{name}/config.yaml` parsing, with `validate` and `to_agent_config` methods.
- `AGENT_CONFIG_FILENAME` and `DEFAULT_AGENT_SYSTEM_PROMPT_FILE` constants.
- `DiskAgentConfig::system_prompt_file` helper with default fallback.
- `PathsConfig::agents` path helper for agent directory resolution.

## [0.1.17] - 2026-02-19

### Added
- `HookEvent` enum with ten variants (`PreToolUse`, `PostToolUse`, `PostToolUseFailure`, `SessionStart`, `SessionEnd`, `UserPromptSubmit`, `Notification`, `Stop`, `SubagentStart`, `SubagentStop`).
- `HookCategory` enum (`System`, `Custom`) defaulting to `Custom`.
- `DiskHookConfig` struct for on-disk `services/hooks/{id}/config.yaml` parsing with typed `HookEvent` and `HookCategory` fields.
- `HOOK_CONFIG_FILENAME` constant.
- `HookEventsConfig::matchers_for_event` method bridging struct fields and the `HookEvent` enum.
- `post_tool_use_failure` field on `HookEventsConfig`.
- `McpServerType` on `McpServerConfig` and a `remote_endpoint` field for external MCP servers; `McpServerType` derives `Default` (= `Internal`) and `Copy`.

### Changed
- `parse_permissions` uses `map` + `collect` instead of `filter_map` that silently swallowed errors.

## [0.1.16] - 2026-02-18

### Added
- `DiskSkillConfig` struct for on-disk `config.yaml` skill format with a `content_file` method.
- `strip_frontmatter` shared utility for stripping markdown YAML frontmatter.
- `SKILL_CONFIG_FILENAME` and `DEFAULT_SKILL_CONTENT_FILE` constants.
- `PluginVariableDef` struct with `name`, `description`, `required`, `secret`, and `example` fields.
- `variables`, `license`, and `depends` fields on `PluginConfig`.

## [0.1.15] - 2026-02-17

### Breaking
- **Breaking:** `PluginComponentRef.source` is now `ComponentSource` and `PluginComponentRef.filter` is now `Option<ComponentFilter>`. Migrate by replacing string literals with the new enum variants.

### Added
- `ComponentSource` enum (`Instance`, `Explicit`) replacing raw string source fields on `PluginComponentRef`.
- `ComponentFilter` enum (`Enabled`) replacing raw string filter fields on `PluginComponentRef`.
- `PluginConfig`, `PluginConfigFile`, `PluginComponentRef`, `PluginScript`, and `PluginAuthor` types.
- `HookEventsConfig`, `HookMatcher`, `HookAction`, and `HookType` types for Claude Code hook configuration.
- `plugins` path accessor on `ProfilePaths`.

## [0.1.14] - 2026-02-11

### Added
- `external_database_url` and `internal_database_url` fields on `Secrets`.
- `Secrets::load_from_path` for loading secrets from an arbitrary file path.
- `Secrets::effective_database_url` resolving the correct URL based on the external access flag.
- `external_database_url` and `internal_database_url` support in `SecretsBootstrap` env var loading and key lookup.

## [0.1.13] - 2026-02-11

### Added
- `allow_registration` field on `SecurityConfig` (default `true`) controlling WebAuthn passkey registration visibility.
- `allow_registration` field on `Config`, wired from profile security settings.

## [0.1.12] - 2026-02-10

### Added
- `SecurityHeadersConfig` struct with configurable HSTS, frame options, content type options, referrer policy, permissions policy, and CSP.
- `security_headers` field on `ServerConfig` and `Config`.
- `refresh_token` grant type in `OAuthServerConfig::supported_grant_types`.

### Changed
- `RouteClassifier` no longer special-cases `/vite.svg` for static asset detection.

## [0.1.10] - 2026-02-08

### Added
- `ContentRouting::resolve_slug` method with a default `None` implementation.
- `ContentRouting` implementation for `ContentConfigRaw`.
- `extract_slug_from_pattern` helper for URL pattern slug extraction.
- `ContentRouting` blanket impl for `Arc<T>` where `T: ContentRouting`.

### Changed
- `RouteClassifier` accepts an optional `ContentRouting` provider.

## [0.1.9] - 2026-02-05

### Added
- `MarkdownFrontmatter` struct for YAML frontmatter in markdown responses, with builder methods for description, author, published_at, tags, and url.
- `MarkdownResponse` struct combining frontmatter and body.
- `ContentNegotiationConfig` struct for server content negotiation settings.

### Changed
- `ServerConfig` now carries a `content_negotiation` field.

## [0.1.4] - 2026-02-04

### Breaking
- **Breaking:** `JwtAudience` is no longer `Copy` because it now contains a `Resource(String)` variant. Migrate by passing `&JwtAudience` (e.g. to `JwtClaims::has_audience`, whose signature changed accordingly).

### Added
- `capabilities` module with MCP UI extension types.
- `McpExtensionId` enum.
- `McpAppsUiConfig` struct.
- `ToolVisibility` enum with `Model` and `App` variants.
- `McpCspDomains` struct with builder for CSP domain configuration.
- `McpResourceUiMeta` struct for resource UI metadata.
- `JwtAudience::Resource(String)` variant for RFC 8707 resource indicators.
- `WWW-Authenticate` header with `resource_metadata` on all 401 responses for MCP OAuth 2.1 compliance.

### Changed
- `Secrets::get` uses `char::is_uppercase` as a method reference.
- Removed doc comments from `ToolUiConfig` methods per coding standards.

## [0.1.3] - 2026-02-03

### Added
- `ActivityRequest` and `ActivityData` types for cloud activity tracking.
- `ApiPaths::CLOUD_ACTIVITY` endpoint constant.
- `ApiPaths::ACTIVITY_EVENT_LOGIN` and `ApiPaths::ACTIVITY_EVENT_LOGOUT` event-type constants.

### Removed
- `WebhooksConfig` and `UserEventsWebhookConfig` from profile configuration.
- `webhooks` field on `Profile`.

## [0.1.2] - 2026-02-03

### Added
- `AiResponse::with_streaming` builder method marking responses as streaming.

## [0.1.1] - 2026-02-03

### Removed
- **Breaking:** `credentials_path` and `tenants_path` fields on `CloudConfig`, plus `Profile::credentials_path` and `Profile::tenants_path`. Migrate by resolving these paths via `ProjectContext` typed paths.

### Changed
- Secrets and profile loading use explicit `map_or_else` patterns in place of `unwrap_or_default`.

## [0.1.0] - 2026-02-02

### Changed
- First stable release; all workspace crates aligned at 0.1.0.

## [0.0.14] - 2026-01-27

### Added
- `ToolUiConfig` struct for configuring UI metadata on MCP tool definitions.
- `ToolUiConfig::to_meta_json` method emitting UI metadata JSON.

## [0.0.13] - 2026-01-27

### Changed
- `Part` enum match arms use `Self::` instead of the type name for clippy compliance.

## [0.0.11] - 2026-01-26

### Breaking
- **Breaking:** `ToolResponse::to_json` and the `Artifact::to_json_value` trait method now return `Result<JsonValue, serde_json::Error>` instead of silently returning `Null` on error. Migrate by handling the `Err` arm at call sites.

## [0.0.7] - 2026-01-23

### Breaking
- **Breaking:** `RotateCredentialsResponse` now returns `internal_database_url` and `external_database_url` instead of a single `database_url` field. Migrate by reading the appropriate URL for the caller's access path.

## [0.0.4] - 2026-01-23

### Added
- `tenant_subscription_cancel` API path for subscription cancellation.
- `ExtensionsConfig` struct for profile-based extension enable/disable configuration.
- `extensions` field on `Profile`.
- `is_masked_database_url` helper for detecting masked credentials.

### Fixed
- Schema validation now handles VIEW-based schemas.
- Migration system infrastructure added.

## [0.0.2] - 2026-01-22

### Changed
- Schemas are registered per-domain via the `Extension` trait; centralized loaders in `systemprompt-loader` are gone.

### Fixed
- `include_str!` paths no longer point outside the crate directory, so the crate compiles standalone when downloaded from crates.io.

## [0.0.1] - 2026-01-21

- Initial release.
