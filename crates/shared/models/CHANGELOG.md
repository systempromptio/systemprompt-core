# Changelog

## [0.41.0] - 2026-08-28

### Added

- `bridge::manifest::CoworkArtifactBundleManifest` and `CoworkArtifactBundleRecord` describe a plugin bundle's `artifacts/manifest.json`.

### Changed

- `ServicesConfig::validate` rejects an enabled plugin, skill or marketplace whose `mcp_servers.include` names a disabled MCP server, and a skill naming an unknown one.

## [0.40.0] - 2026-08-26

### Added

- `SkillConfig` gains an optional `hosts` targeting list consumed by the marketplace bundle emitters.

### Changed

- **Breaking:** `CanonicalRequest::flatten_text` is replaced by `flatten_parts`, returning `(path, text)` pairs (`system`, `messages[i].<role>`, `forwarded.<leaf.path>`) so governance can attribute matches to their true source.

## [0.39.0] - 2026-08-25

### Added

- `ModelGovernance` (`european`, `no_retain`): a data-governance posture declared on a provider entry (`ProviderEntry::governance`) or per model (`ProviderModel::governance`), resolved by `ProviderEntry::effective_governance`.
- `RouteRequirements` on `GatewayRoute::requires`: a route can demand `european` and/or `no_retain` of its target; `GatewayConfig::validate` rejects a route whose reachable provider/model does not satisfy the requirement (`GatewayProfileError::RouteGovernanceUnsatisfied`).

### Changed

- **Breaking:** the three new fields land on structs that are not `#[non_exhaustive]`, so struct-literal construction breaks. Add `requires: None` to `GatewayRoute`, `governance: ModelGovernance::default()` to `ProviderEntry`, and `governance: None` to `ProviderModel`. Deserialization is unaffected — every new key is `#[serde(default)]`.
- **Breaking:** `GatewayProfileError::RouteGovernanceUnsatisfied` is a new variant, so an exhaustive match over the enum no longer compiles.
- A profile that uses `requires:` or `governance:` is rejected by a 0.38 binary: the parent structs carry `deny_unknown_fields`. The compatibility is one-way — 0.38 profiles load on 0.39, not the reverse.

## [0.38.0] - 2026-08-25

### Added

- `text::floor_char_boundary` truncates to a char boundary without panicking on a multi-byte split.

### Changed

- `mime` resolves canonical extensions from a `CANONICAL_EXTENSIONS` table instead of a special-cased branch, so a type and its extension are declared together.

## [0.36.0] - 2026-08-23

### Added

- macOS backend for `subprocess`: `live_pid_is_subprocess` and `is_zombie` now read the target process through `sysctl(KERN_PROCARGS2)` and `proc_pidinfo` rather than returning a fail-closed `false`, so a child spawned by this installation can be recognised — and therefore reclaimed — on Darwin. A child running under Apple's hardened runtime withholds its environment and still never verifies; the `darwin` module documents that gap.
- `subprocess::identity_verification_supported`, reporting whether the running platform can establish a child's identity at all, so callers can distinguish "not ours" from "unknowable here".
- `subprocess::environ_from_procargs2`, the pure parse of the Darwin argument-and-environment blob. It skips `argv` by count rather than scanning past it: entries are matched whole, so a command line such as `env MCP_SERVICE_ID=files …` would otherwise read as a marked environment.

### Changed

- `subprocess` is now a module directory with one file per platform backend. Every public path is unchanged.
- The module documentation no longer claims child supervision is Linux-only. Identity and reap checks work on Linux and macOS; parent-death prevention remains `prctl(PR_SET_PDEATHSIG)` and therefore Linux-only, since macOS offers no equivalent that survives `execve` and the alternatives need cooperation from child binaries this supervisor does not control.

## [0.33.0] - 2026-08-20

### Breaking

- **Breaking:** `SecurityConfig` (and `Config`) gain a `login_page_url: Option<String>` field. Migrate by adding `login_page_url: None` to struct-literal constructions.

## [0.32.1] - 2026-08-19

### Added

- `SystemAdminConfig::email` (`system_admin.email`, typed `Email`, optional in YAML): the explicit owner email `admin bootstrap` uses when it first creates the owner row, replacing the synthesized `{name}@localhost` fallback. Mirrored on runtime `Config` as `system_admin_email`; `Profile::from_env` reads `SYSTEM_ADMIN_EMAIL`.

## [0.32.0] - 2026-08-18

### Breaking

- **Breaking:** The cloud signup/management wire types are removed from `api::cloud`: `Checkout*`, `ProvisioningEvent(_Type)`, `ActivityRequest`/`ActivityData`, `CustomDomainResponse`/`DnsInstructions`/`SetCustomDomainRequest`, `SetExternalDbAccessRequest`/`ExternalDbAccessResponse`, `ListSecretsResponse`, and `CloudLogEntry`/`CloudLogsResponse`. `ApiPaths` loses the checkout/activity constants and the `tenant_events`/`tenant_restart`/`tenant_retry_provision`/`tenant_external_db_access`/`tenant_subscription_cancel`/`tenant_custom_domain` helpers.

### Added

- `McpExtensionId::Tasks` names the `io.modelcontextprotocol/tasks` extension.

### Changed

- Built against `rmcp` 3.1.3; the re-exported `CallToolResult` and related types follow.

## [0.31.0] - 2026-08-18

### Breaking

- **Breaking:** `bridge::manifest::SignedManifest` no longer carries a `signature` field; the manifest endpoint returns the new `SignedManifestEnvelope { payload, signature }`, where `payload` is the manifest's JCS-canonical JSON signed byte-for-byte. Migrate by verifying the signature over `payload` before deserialising it.

### Added

- `bridge::manifest::MANIFEST_SCHEMA_VERSION` and `SignedManifest::min_schema_version` declare the oldest schema level that can safely consume a manifest, so consumers refuse with an upgrade message instead of a signature error when a semantic break lands.

## [0.30.1] - 2026-08-07

### Added

- `ai::ToolCallResult` carries the tool result's wire `_meta` (`meta` field + `with_meta`), so execution state can reconstruct a faithful `CallToolResult`.

## [0.30.0] - 2026-08-07

### Breaking

- **Breaking:** `ExecutionMetadata::to_meta` is replaced by `to_object`, which returns the bare field map. On the wire the fields now travel under the new `artifacts::EXECUTION_META_KEY` (`io.systemprompt/execution`); `ToolResponse` remains the storage envelope for persisted artifacts and no longer appears on the wire.
- **Breaking:** `ToolResponse::schema()` is removed. The storage envelope must not be advertised as a tool's wire schema — advertise the typed artifact's schema via `McpOutputSchema::validated_schema()` instead.

### Added

- `mcp::ClientProfile` captures what a client declared during `initialize` — protocol version, implementation name, negotiated extensions — with `supports_ui` and `supports_structured_content` deciding which wire pieces it can accept.
- `CliArtifact::text_body` returns the plain-text body of text-bearing variants.
- `gateway.bridge_releases` (`BridgeReleasesSpec`) configures the desktop-bridge self-updater feed: source repo, token env-var name, tag prefix, optional pinned version, and the platform→asset map. Absent means the update endpoints report "not configured" and bridges never see an update.
- `services::BridgePolicyConfig` (`bridge_policy:` in a services YAML) and `SignedManifest::allow_claude_ai_connectors` carry the instance policy that re-allows claude.ai first-party connectors under the bridge's managed-MCP enforcement. The field is `#[serde(default)]`, so manifests from older servers still verify.

### Changed

- `text/html;profile=mcp-app` is defined once as `mcp::MCP_APP_MIME_TYPE`; `mcp::apps::RESOURCE_MIME_TYPE` aliases it.

### Fixed

- `GatewayConfig::validate` costs a rewrite route by its `upstream_model` rather than by pattern-matching the catalog. A route such as `model_pattern: gemini-*` rewritten to a priced `gpt-oss-120b` dispatches every request to that one model and bills correctly at runtime, but was rejected at boot with `RouteReachesNoPricedModel`; an unpriced or catalog-absent upstream model is still rejected.

## [0.29.0] - 2026-08-05

### Added

- `AiRequestBuilder::with_system_prompt` sets the request's system prompt.

## [0.28.0] - 2026-07-31

### Breaking

- **Breaking:** `CanonicalRequest` gains `forwarded_surface: ForwardedSurface`, holding every string in the bytes about to be forwarded upstream. `CanonicalRequest::flatten_text` and `message_units` include it, so a scanner reading either covers content the canonical form cannot represent. Migrate by adding `forwarded_surface: ForwardedSurface::default()` at each construction site; a caller that does not send through the gateway leaves it empty. `CanonicalRequest` now derives `Default`.
- **Breaking:** `wire::anthropic::event_from_sse` is replaced by `events_from_sse`, which returns a `Vec<CanonicalEvent>` rather than an `Option`. One `message_delta` frame carries both the terminal `stop_reason` and the final cumulative `usage`, and the old signature could return only one of the two — it returned the stop and dropped the usage, which is the sole place Anthropic reports real token counts for a stream. Migrate by iterating the returned events instead of matching one option.
- **Breaking:** `ModelPricing` gains `cache_read_per_million` and `cache_write_per_million`. A struct built by literal needs the two extra initialisers; `..ModelPricing::default()` preserves today's arithmetic. Both are `#[serde(default)]`, so a catalog written before this release still parses — its cached tokens simply bill at zero until rates are supplied.
- **Breaking:** `OAuthRequirement` gains `ema: bool` (`#[serde(default)]`), declaring the MCP Enterprise-Managed Authorization extension on a server's protected-resource metadata.
- **Breaking:** `CanonicalEvent::UsageDelta` carries a `CanonicalUsageUpdate` rather than a `CanonicalUsage`. The new type holds four `Option<u32>` counts, so "the frame never stated this" and "the frame reported zero" stop being the same value; `apply_to` folds an update onto a running `CanonicalUsage` and rederives the total, and `is_empty` reports a frame that stated nothing. The streaming codecs emit `None` for a count their frame omits — Anthropic's `message_delta` typically states `output_tokens` alone.

### Added

- `wire::inspect`: `string_leaves` collects every string in a JSON body, `sse_string_leaves` does the same across concatenated SSE frames under one shared budget, and `SurfaceBudget` bounds the walk on depth, leaf count, total bytes, and per-leaf size. The walk is iterative — a recursive one over caller-controlled JSON is a stack-overflow primitive well inside the body limit — and a budget stop marks the result truncated rather than reporting a complete surface.
- `wire::anthropic::strip_user_id` removes `metadata.user_id` from a request body object, dropping `metadata` once it empties rather than sending `{}`. Both the canonical and byte-passthrough lanes strip through this one function.
- `ModelPricing::is_billable` reports whether rates can produce a non-zero bill: a token model needs both an input and an output rate, while an image model prices per image and is legitimately zero on both token rates.
- `GatewayConfig::validate` rejects a route that cannot be costed. A route carrying its own `pricing:` override must make that override billable; otherwise every registry model the pattern reaches must carry usable rates, and the pattern must reach at least one — which is what catches a glob route aimed at a catalog that has fallen behind the models actually in use. `GatewayProfileError` gains `RouteModelUnpriced` and `RouteReachesNoPricedModel`. Uncosted inference is a configuration bug rather than a runtime warning: the request bills at zero and the gap stays invisible until someone reads the ledger.
- `McpExtensionId::EnterpriseManagedAuth` names `io.modelcontextprotocol/enterprise-managed-authorization`, previously spelled as a bare string constant at each use site.
- The default provider catalog carries cache read and write rates on every Anthropic, OpenAI, and Gemini model, adds `claude-opus-5`, `claude-sonnet-5`, `claude-fable-5`, and `claude-haiku-4-5`, and gives the dated model ids `aliases` so `claude-opus-4-5` and `claude-opus-4-5-20251101` resolve to one entry. Sonnet 4.6's context window and output ceiling are corrected to 1M/128k.
## [0.27.0] - 2026-07-29

### Breaking

- **Breaking:** `AppPaths::from_profile` and `SystemPaths::from_profile` take a `PathResolution`; derive it with the new `Profile::path_resolution()`. Cloud profiles resolve lexically — container paths are no longer canonicalized against the local filesystem — and `PathError` gains `NotAbsolute` for a relative path under lexical resolution.
- **Breaking:** `default_bootstrap_jobs()` no longer includes `database_cleanup`; deployments relying on the boot-time sweep must list it in `scheduler.bootstrap_jobs` explicitly.
- **Breaking:** `SchedulerConfig::with_system_admin()` sets `enforce` on `cleanup_anonymous_users`, `cleanup_empty_contexts`, and `database_cleanup`, matching the jobs' new observe-by-default behaviour.

### Added

- `JobConfig.parameters` (`#[serde(default)]`, string map) with a `with_parameters` builder; the field rustdoc tables the keys the core jobs read.
- `RequestScope`: per-request scoping identity (ordered key/value pairs) carried from middleware to the scoped database transactions in `systemprompt-database` — the transport for pooled multi-tenancy dimensions such as the requesting user's organization.

## [0.26.0] - 2026-07-28

### Breaking

- **Breaking:** `ChartArtifact` serializes `chart_type`, `title`, `x_axis_label`, `y_axis_label`, `x_axis_type`, and `y_axis_type` with the payload, and the fields are public. They were `#[serde(skip)]` and only ever surfaced through the schema's `x-chart-hints` block, which nothing on the stored-artifact path read. The `x-chart-hints` block is gone from `to_schema`; the fields appear in `properties` instead. Payloads stored before this release deserialize to the defaults.
- **Breaking:** `DashboardArtifact.hints` serializes with the payload and is public, and `DashboardHints::generate_schema` is deleted along with the schema's `x-dashboard-hints` block — layout travels in the artifact itself rather than in a side channel the renderer never received.
- **Breaking:** `CanonicalContent::Thinking` gains `id` and `encrypted_content`, and `ContentBlockKind::Thinking` gains `id`. OpenAI Responses reasoning items carry a provider id and an opaque `encrypted_content` blob that must be replayed verbatim for stateless reasoning continuity — the canonical model previously had no channel for either, so the gateway emitted id-less reasoning items upstream and lost continuity every turn. `CanonicalEvent` gains an `EncryptedContentDelta` variant carrying the blob when it arrives at `output_item.done`. Migrate constructors by supplying `None` and exhaustive matches with a no-op arm.

### Added

- `SectionType::Text`, `TextSectionData`, `TimelineSectionData`, and `TimelineEvent`: the free-text and timeline section bodies the dashboard renderer now consumes. `Timeline` existed in the taxonomy but had no data shape.

### Fixed

- Gemini thought parts round-trip. `GeminiPart::Text` now models the part-level `thought` flag and `thoughtSignature`, so thought summaries parse to `CanonicalContent::Thinking` (streamed as real thinking blocks with `ThinkingDelta`/`SignatureDelta`) instead of leaking to clients as ordinary answer text with the signature silently dropped. The request encoder replays `Thinking` as `{"thought": true}` parts with signatures — the documented "return the entire response with all parts back" contract — where it previously discarded thinking entirely, and `thinkingConfig.includeThoughts` is set whenever thinking is enabled so Gemini returns thought parts at all.
- The Anthropic upstream body never carries an unsigned thinking block. Anthropic rejects a replayed thinking block without its signature; a block arriving signatureless (cross-provider history, or a client that dropped it) is now omitted — and a message reduced to zero blocks is dropped rather than sent empty — degrading reasoning continuity instead of failing the request.
- OpenAI Responses reasoning items are emitted one per `Thinking` part with their provider `id` and `encrypted_content` (previously all parts collapsed into a single id-less summary item, which the API rejects), and reasoning-enabled request bodies send `store: false` so encrypted reasoning content is returned for stateless replay. A `Thinking` part with no provider id emits nothing rather than a malformed item.
- The Anthropic upstream request body no longer carries the gateway's vendor-extension fields. `build_request_body` shared its block renderer with the client-facing render, so `signature` on `tool_use` and `structuredContent`/`_meta` on `tool_result` — fields the gateway adds for its own clients — went to the real Anthropic API, which rejects unknown keys in content blocks. The client-facing `content_to_anthropic_block` still emits them; the upstream body does not.
- Gemini `functionResponse.name` carries the declared function name rather than the gateway-minted `tool_use` id. Gemini's function call has no id, so the canonical `ToolResult` only holds the minted one; the encoder now recovers the name from the matching `ToolUse` earlier in the same replayed history, falling back to the id only when no match exists.

## [0.25.0] - 2026-07-27

### Added

- `CanonicalRequest::latest_message_text` flattens the newest message of a role, and `CanonicalRequest::message_units` flattens the system prompt and each message into its own string. A safety scanner that judges `flatten_text` re-reads the whole conversation on every turn, and a detector that slides a window over it sees two unrelated messages as one; these are the primitives that let a caller scope to the newest turn and respect message boundaries.
- `systemprompt_models::mime` is the single source of truth for extension↔MIME mapping. `from_path` and `from_extension` give the parameterless essence to store or validate against an allowlist, `http_content_type` gives the served form carrying `charset=utf-8` on the `text/*` types, `extension_for` inverts the mapping tolerating parameters and aliases, and `essence_of` strips parameters from a client-supplied type. Six independent tables previously disagreed about `woff`, `yaml`, and whether a charset was emitted, and a format missing from one of them was served as `application/octet-stream`.
- `subprocess::spawn_supervised` is the sanctioned way to start an agent or MCP child. It spawns every child from one dedicated, never-joined thread and arms `prctl(PR_SET_PDEATHSIG, SIGTERM)` in the forked child, so a supervisor that is `SIGKILL`ed or panics no longer strands children holding ports 8080/5010/9101/9102 for the next boot to reclaim. The dedicated thread is load-bearing: the death signal fires when the *forking thread* exits, so forking from a tokio worker would tie a live agent's lifetime to whichever worker happened to poll the spawn.

### Changed

- Child supervision is documented as Linux-only, matching where the server runs. The identity and reap checks read `/proc` and the death signal is `prctl`; on other platforms `live_pid_is_subprocess` now logs a WARN naming the consequence — the process will not be signalled and must be stopped by hand — instead of silently returning `false`.

## [0.24.0] - 2026-07-26

### Breaking

- **Breaking:** `McpToolResultMetadata::to_meta`/`from_meta` and `ExecutionMetadata::to_meta` use `rmcp::model::MetaObject` instead of `rmcp::model::Meta`, which rmcp 3.0 demoted to a deprecated alias. Migrate by renaming the type at call sites; construction and `Deref` behaviour are unchanged.

## [0.23.0] - 2026-07-24

### Breaking

- **Breaking:** `CardSection.content` is a `serde_json::Value` rather than a `String`, so a card section can carry structured data that serializes as real nested JSON instead of a JSON-encoded string. Migrate by building sections with `CardSection::new` (unchanged, wraps a string) or the new `CardSection::value`, and by reading the display form through `CardSection::content_display()`.
- **Breaking:** `AgentSkillConfig` and `AgentCardConfig.skills` are removed. A2A `card.skills` has been computed at serve time from `metadata.skills` and the on-disk skill catalog since the catalog refactor, so authoring the field was already a no-op. Migrate by deleting `card.skills` from agent YAML.

### Added

- `ApiPaths::GATEWAY_PUBLIC_BASE` (`/api/public/gateway`), the unauthenticated gateway path prefix.
- `JobConfig.enforce` (default `false`) and `JobConfig::with_enforce`, the opt-in for destructive scheduler-job actions.

## [0.22.0] - 2026-07-20

### Added

- `mcp::apps` provides typed bindings for the MCP Apps extension (SEP-1865): `UiMethod`, `McpUiToolMeta`, `SizeChangedParams`, `UiMessageParams`, `UiInitializeParams`, the extension constants, and `ui_method_js_constants` for projecting method names into browser code.

### Fixed

- `McpCspDomains` serializes to the `connectDomains`, `resourceDomains`, `frameDomains`, and `baseUriDomains` field names the MCP Apps schema defines, so hosts apply the declared content-security policy.

### Breaking

- **Breaking:** `ServerConfig.trusted_proxies` is now `Vec<IpNet>` (was `Vec<String>`), parsed and validated when the profile loads; an invalid CIDR entry fails boot. Migrate by expressing entries in CIDR notation.
- **Breaking:** `SecurityHeadersConfig.frame_options` is now the `FrameOptions` enum (was `String`). Migrate by using `DENY` or `SAMEORIGIN`.

### Changed

- Profile validation rejects URL fields that are not `http(s)` and CORS origins that include a path, query, or fragment.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.21.0] - 2026-07-16

### Added

- `none_if_blank` normalizes optional env- or flag-sourced values, treating blank and whitespace-only strings as absent.

### Fixed

- `Secrets::parse` drops empty and whitespace-only string entries alongside nulls, so blank provider keys no longer count as configured.

## [0.20.0] - 2026-07-15

### Breaking

- `UserContext` and `UserContextWithStats` gain a `kind: ContextKind` field; struct-literal constructions and exhaustive destructurings must add it. `ContextKind` (`User` | `CliSession`) is the new discriminator for `user_contexts` rows.

### Added

- Optional `sqlx` feature deriving `sqlx::Type` for DB-persisted enums (`ContextKind`), so compile-time-verified queries decode them directly.

## [0.19.0] - 2026-07-02

### Breaking

- rmcp is upgraded to 2.x; helpers that traffic in tool-result content (`ToolResultFormatter`, `CallToolResultExt`) now operate on `rmcp::model::ContentBlock` in place of the removed `Content`/`RawContent` pair. Migrate matches on `content.raw` to match `ContentBlock` directly (the enum is `#[non_exhaustive]`, so include a `_` arm).
- The minimum supported Rust version is 1.94.

### Added

- `SignedManifest.artifacts`: a signed manifest section of Cowork Artifacts-library HTML documents (`ArtifactEntry`, keyed by the new `LibraryArtifactId`). These are Cowork-native library documents, distinct from the in-chat MCP artifacts.
- `DiskArtifactConfig` (`services/artifacts/<id>/config.yaml`) describing an on-disk artifact: id, owning `plugin_id`, `mcp_tools`, HTML content file, and enablement.
- `MarketplaceConfig.artifacts`: an include list scoping which artifacts a marketplace ships.

## [0.17.0] - 2026-06-24

### Breaking

- `JobConfig.owner` is now `Option<UserId>` and `JobConfig::new` no longer takes an owner; a job with no owner runs as the profile `system_admin`. Migrate by calling `JobConfig::new(name)` and adding `.with_owner(id)` only where a specific owner is required.
- `SchedulerConfig::with_system_admin` no longer takes a `&SystemAdmin` argument. Migrate by calling `SchedulerConfig::with_system_admin()`.

### Added

- `SlackAppConfig` and `TeamsAppConfig` service-configuration blocks for the Slack and Teams messaging surfaces.
- Gateway routes accept an optional `when` block (`RouteMatch`) of request-shape predicates — `requires_tools`, `min_tools`, `thinking`, `min_reasoning_effort`, `stream`, `min_input_tokens`, `response_format` — that narrow a route beyond its model glob. A route without a `when` block matches purely on model name, exactly as before. Contradictory predicate sets (a zero `min_tools`, or `requires_tools: false` with a positive `min_tools`) are rejected during profile validation.

## [0.16.1] - 2026-06-22

### Added

- `SecurityConfig.id_jag_ttl_secs` and `Config.id_jag_ttl_secs` set the lifetime of minted ID-JAG assertions; defaults to `DEFAULT_ID_JAG_TTL_SECS` (300s).
- `TrustedIssuer` gains `typ_allowlist`, `allowed_client_ids`, and `can_issue_id_jag` for the Enterprise-Managed Authorization (EMA) flow. All default to empty/false, so existing profile YAML is unaffected.

## [0.16.0] - 2026-06-22

### Breaking

- Error enum tuple variants that wrapped a bare message string are now struct variants with a named `message` field; match arms and constructors change from `Error::Foo(msg)` to `Error::Foo { message: msg }`.
- The minimum supported Rust version is 1.88.

### Added

- `services::frontmatter::split_frontmatter` and `Frontmatter`: line-anchored YAML frontmatter splitting, the canonical parser for every frontmatter consumer in the workspace.
- `mcp::ExternalAuth` plus `Deployment.external_auth`/`headers` and `McpServerConfig.external_auth`/`headers`: an external MCP server declares a relative `token_endpoint` accessor from which core resolves a per-user third-party bearer to inject in place of the systemprompt credential. `McpServerConfig::call_url` returns the configured remote endpoint for external servers and the gateway-derived URL otherwise. `external_auth`/`headers` are rejected on `internal` servers at config-load time.

### Fixed

- `strip_frontmatter` no longer treats `---` sequences inside the body — markdown table separator rows, horizontal rules, or mid-line dashes — as frontmatter delimiters; content that does not open with a `---` line is returned unchanged. Previously a frontmatter-less document containing a table separator row lost everything up to that row.

## [0.14.1] - 2026-06-01

### Removed

- `services::ai::AiProviderConfig.default_image_resolution` is removed. The field was parsed and stored but never read by any provider client; image resolution is governed by `ModelCapabilities.image_resolution_config` on the registry model.

## [0.14.0] - 2026-06-01

### Breaking

- Adds `profile.providers` (`ProviderRegistry`, `ProviderEntry`, `ProviderModel`, `WireProtocol`) as the single source of upstream connectivity and the model catalog. `GatewayConfig` drops its embedded catalog — only `routes` and `default_provider` remain — and the standalone `profile/gateway/catalog.rs` / `GatewayModel` are removed; model identity, aliases, `upstream_model`, pricing, capabilities, and limits now live on `ProviderModel`. `ProviderRegistry::validate` is the authority for connectivity (unique provider names, SSRF-guarded endpoints, globally-unique model ids/aliases).
- Adds the provider wire codecs and the provider-neutral canonical model under `wire/` (`wire::{anthropic, openai_chat, openai_responses, gemini, canonical}`), folding in the former `systemprompt-ai-wire` crate. Buffered Anthropic, OpenAI Chat, and OpenAI Responses replies parse into typed `#[derive(Deserialize)]` structs.
- `services::ai::AiConfig` references providers by `ProviderId` and no longer carries connectivity; `validators::ai` validates the AI config's references against the registry.

### Added

- The canonical model carries provider evidence and accounting uniformly: `CanonicalResponse` gains `grounding` (`Grounding` / `GroundedSource` — web-search sources and the queries that produced them), `code_execution` (`CodeExecutionOutput`), and `raw_finish_reason`; `CanonicalUsage` gains `cache_read_tokens`, `cache_creation_tokens`, and `total_tokens`; `CanonicalRequest` gains `presence_penalty`, `frequency_penalty`, and a `code_execution` flag; and `ImageSource` gains an optional `detail` (`ImageDetail`) with `ImageSource::Url` now a struct variant. Each `wire::*` codec extracts these fields from the corresponding provider format.

## [0.13.1] - 2026-06-01

### Added

- The gateway profile config gains an optional `default_provider: Option<ProviderId>` on `GatewayConfigSpec` / `GatewayConfig`. `GatewayConfig::resolve_route` returns the explicit route match or a synthesized catch-all route to the default provider as a `Cow<GatewayRoute>`, `is_model_exposed` reports every model as exposed while a default provider is set, and `GatewayConfig::validate` rejects a default provider absent from the catalog via the new `GatewayProfileError::DefaultProviderNotInCatalog` variant.

### Changed

- `bridge::plugin_bundle` now holds `PluginManifest` and the bundle well-formedness predicate (moved from `entry/api`) as the single definition shared with the bridge and CLI.

## [0.13.0] - 2026-05-28

### Breaking

- `MarketplaceConfig.mcp_servers` is now `PluginComponentRef { source, include, exclude }` instead of a flat `Vec<String>`. Tenants must rewrite YAML from `mcp_servers: [a, b]` to `mcp_servers: { source: explicit, include: [a, b], exclude: [] }`. The flat-list form is rejected at config-load time with a serde "expected struct, found sequence" error. `ServicesConfig::validate_marketplace_bindings` now reads `marketplace.mcp_servers.include` and resolves each id against the top-level `services.mcp_servers` catalogue.
- All remaining entity-id reference lists across the services config now use `PluginComponentRef` for shape uniformity: `PluginConfig.mcp_servers`, `PluginConfig.content_sources`, `SkillConfig.mcp_servers`, `SkillConfig.assigned_agents`, `DiskAgentConfig.mcp_servers`, `DiskAgentConfig.skills`, `AgentMetadataConfig.mcp_servers`, `AgentMetadataConfig.skills`, `bridge::manifest::AgentEntry.mcp_servers`, and `bridge::manifest::AgentEntry.skills`. Authoring YAML must move from flat lists (`mcp_servers: [a, b]`) to the object form (`mcp_servers: { include: [a, b] }`). `PluginComponentRef` now derives `PartialEq`/`Eq` so it can appear inside `#[derive(PartialEq)]` runtime info structs.
- `TaskMetadata.extensions` is now `serde_json::Map<String, Value>` instead of `Option<…>`. The field is `#[serde(flatten)]`, under which `skip_serializing_if` is a no-op and a flattened `Option<Map>` always deserialises back to `Some({})` — so the previous type could never round-trip (`None` became `Some({})`). An empty map carries the same "no extensions" meaning and round-trips cleanly. Callers reading the field drop the `Option` (`metadata.extensions` is the map directly); `with_extension` is unchanged.

### Changed

- `bridge_manifest::manifest()` now scopes the manifest's skills, agents, mcp_servers, and plugins to the active marketplace's `MarketplaceConfig.<entity>.include` lists before RBAC filtering. `MarketplaceConfig` was previously parsed but unused at manifest time. Empty `include:` preserves the global-list fallback for backwards compatibility. All four catalogues are now uniformly authored as `PluginComponentRef` on `MarketplaceConfig`.
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
