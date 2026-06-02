# Changelog

## [0.14.3] - 2026-06-02

### Fixed

- MCP cross-restart session recovery now works after a session's worker has closed. Recovery looked up the persisted `initialize` params with `status = 'active'`, but a worker that ends — after any request, or for every session on a graceful restart — marks its row `closed` while leaving the params intact (only a client `DELETE` clears them). The lookup now keys on the presence of non-null `initialize_params` rather than status, so a streamable HTTP session is restored instead of returning `404 Session not found` and provoking a client reconnect loop. A restored session's row is marked `active` again on its next activity.
- The gateway preserves Gemini's function-call `thoughtSignature` across conversation turns. Gemini 3.x attaches an opaque thought signature to each `functionCall` part and rejects a replayed tool turn whose first function call omits it (`400 INVALID_ARGUMENT: Function call is missing a thought_signature`). The canonical tool-use representation now carries this signature: the Gemini codec captures it from response `functionCall` parts and re-emits it on the next request, and it round-trips through the Anthropic Messages inbound surface (buffered blocks and streamed `tool_use` start frames) alongside the existing extended-thinking signature handling. Tool calls originating from other providers leave the signature unset and are unaffected.
- `infra logs trace list` reports a status for every trace instead of `unknown`. The status was derived solely from a trace's most recent agent task, so traces produced by gateway-proxied AI requests, MCP tool executions, or scheduled-job logs — none of which create an agent task — had no status. The trace-list query now derives a canonical status (`running`, `failed`, `canceled`, or `completed`) from the agent task, AI requests, MCP tool executions, and error logs that share the trace, and the status column is no longer nullable. The `--status` filter consequently matches agent traces as well, whose status was previously stored in a form the filter could not match.

## [0.14.2] - 2026-06-02

### Added

- OAuth dynamic client registration accepts and persists the RFC 7591 `application_type` (defaulting to `"web"`). The field is stored on `oauth_clients` (migration 011), carried on `OAuthClientRow` / `OAuthClient`, and surfaced through registration and the client-config endpoints.
- MCP sessions survive a server restart. A Postgres-backed `SessionStore` records each session's `initialize` params in `mcp_sessions` and restores them on demand, so the streamable HTTP service transparently re-creates a session whose in-memory worker was lost to a restart or eviction instead of returning `404 Session not found` and provoking a client reconnect storm. Persistence is best-effort: a store error degrades to the re-initialize path.
- The gateway propagates Anthropic extended-thinking signatures. A `signature_delta` SSE frame is parsed into a canonical `SignatureDelta` event, accumulated onto the in-flight thinking block, and rendered back out on the Anthropic and OpenAI Responses inbound surfaces.
- `Database::from_pools` and `PostgresProvider::from_pool` build a database handle from already-open `PgPool`s, letting an extension construct core data services from a pool it already holds without re-dialing the database.

### Changed

- Tool-result content carries `structured_content` and `meta` through the wire codecs. When a tool result has structured content, the Gemini codec emits it as the `functionResponse` result, falling back to flattened text otherwise.
- CLI command output is unified behind a single non-generic `CommandOutput`, replacing the former generic `CommandResult<T>` wrapper. The unused artifact-conversion plumbing (`CommandResultRaw`, `ConversionError`, `CliArtifactType`, `RenderingHints`, and the cli conversion module) is removed from `systemprompt-models`.

### Fixed

- Process-signal helpers no longer let an out-of-range PID escalate to a group or broadcast kill. A `u32` PID above `i32::MAX` wraps to a negative value, which `kill(2)` reads as a process group (`-1` broadcasts to every process the caller can signal). The scheduler, agent, and MCP signal helpers now route every PID through `systemprompt_models::subprocess::signalable_pid`, which rejects `0` and out-of-range values so an invalid id becomes a no-op instead of a group or session-wide kill.

## [0.14.1] - 2026-06-02

### Added

- The AI gateway now resolves `policy.safety.scanners` against a scanner registry and **enforces** `policy.safety.block_categories`. Request-phase scanners selected by policy run before the upstream call; if any finding's category is listed in `block_categories`, the request is denied with `403` and an `ai_safety_findings` audit row, with no upstream dispatch. Response-phase scanning remains audit-only (the response is already streaming). Scanners are a Rust extension point: the built-in `heuristic` scanner ships in-tree, and extensions contribute additional scanners via `systemprompt-ai`'s `register_safety_scanner!`. An empty `scanners` list runs nothing — scanning is now fully config-driven.
- Access-control rules can target entities by `*`-glob (`entity_match`) in addition to a literal `entity_id`; each glob is expanded against the entities already in the catalog for that `EntityKind`, one resolved rule per match. Gateway routes are now first-class authz entities — `GatewayState::resolved_route_ids` materialises their content-addressed ids straight from the typed profile, and a new `systemprompt admin config reconcile` command upserts the gateway-route entity rows so glob rules can resolve against them.
- Per-model thinking-budget ceilings. A provider model card may declare `max_thinking_budget`; the gateway clamps a request's extended-thinking / reasoning budget to it before dispatch, so an out-of-range budget no longer makes the upstream reject the call (notably Gemini's `thinkingBudget`). Omitting it leaves budget validation to the provider.
- `admin config gateway` and `admin config catalog` now report when the post-edit authz reconcile could not run (e.g. the database was unreachable during an offline edit): the success output carries a deferral notice so the operator knows the live catalog stays stale until the next app start or a retry.

### Changed

- JSON-Schema handling for provider tool/output schemas is unified. The capability matrices (`ProviderCapabilities`) and the sanitiser (`SchemaSanitizer`) move to `systemprompt-models` (`schema::`) so the gateway wire codecs and the agent-flow provider clients reduce tool schemas through one authority; each `WireProtocol` resolves its matrix via `WireProtocol::schema_capabilities`. `systemprompt-ai` re-exports the types from `services::schema`, but code importing the old `services::schema::{capabilities, sanitizer}` submodules directly must switch to the re-export (or `systemprompt-models::schema`).
- Scheduler job names are validated against the registered `inventory` catalog. A name in `jobs` or `bootstrap_jobs` that was never registered via `submit_job!` now fails startup with `SchedulerError::UnknownJob` instead of being silently skipped, and a bootstrap job with no explicit owner entry defaults to the system admin. `Extension::jobs` is documented as an introspection-only manifest that the scheduler does not consult.

### Fixed

- Process liveness checks now treat a zombie (exited-but-unreaped) child as dead. The MCP orchestrator's `is_process_running` previously reported a defunct server as alive because its PID still answered `kill(pid, 0)`; it now also rejects processes in state `Z`.
- On shutdown, the API confirms that a recorded agent or MCP server PID still belongs to the process it spawned — matching the spawn-time markers in `/proc/<pid>/environ` — before terminating its process group. A stale registry PID that the OS has recycled to an unrelated process is cleared without being signalled, instead of having the reused PID's process group killed. The process-group termination primitive additionally refuses to broadcast to a target that is not its own group leader.

## [0.14.0] - 2026-06-01

### Breaking

- AI provider configuration is unified into a single profile-level **provider registry**. `Profile` gains a `providers` section (`ProviderRegistry`) in which each provider is declared exactly once — its wire protocol, endpoint, credential, extra headers, and the model catalog it serves — and the gateway and AI-service layers now reference providers by `ProviderId` instead of re-declaring connectivity. The gateway no longer owns a model catalog: `profile.gateway` keeps only routing and `default_provider`, while model identity, aliases, `upstream_model`, pricing, capabilities, and limits live once on `ProviderModel`. Tenants must add a `providers:` block and move per-model definitions out of the old gateway catalog into it; the registry is the authority for connectivity validation (unique provider names, SSRF-guarded endpoints, globally-unique model ids/aliases).
- The provider **wire types** — the Anthropic Messages, OpenAI Chat Completions, OpenAI Responses, and Gemini codecs plus the provider-neutral canonical request/response model — are folded into `systemprompt-models` under `wire/`. The standalone `systemprompt-ai-wire` crate is removed; depend on `systemprompt-models` (`wire::*`) instead.
- `AiService::new` takes the resolved provider registry: `AiService::new(&db_pool, &registry, &ai_config, tool_provider, session_provider)`. The AI service resolves provider connectivity from the registry, and `services/ai/config.yaml` declares only the agent default provider/model and per-provider overrides rather than a private connectivity block.

### Added

- The provider-neutral canonical model carries the evidence and accounting that responses now expose uniformly across providers: web-search **grounding** (sources with URI/title/snippet/relevance plus the queries that produced them), server-side **code-execution** output, and richer **usage accounting** (cache-read and cache-creation tokens alongside input/output and a total). Requests gain `presence_penalty` / `frequency_penalty` sampling controls and a `code_execution` toggle, and image inputs carry an optional `detail` hint (`ImageSource::Url` is now a struct variant). These fields are extracted by the Anthropic, OpenAI Chat, OpenAI Responses, and Gemini codecs and mapped to and from the AI domain by the new `canonical_bridge`.

### Changed

- Gateway outbound dispatch is driven by the provider registry and the relocated wire codecs. A Gemini outbound adapter is added, and the per-protocol request/response/streaming handling that previously lived under the `entry/api` gateway is consumed from `systemprompt-models::wire`.
- Buffered provider replies (Anthropic Messages, OpenAI Chat Completions, OpenAI Responses) are parsed into typed `#[derive(Deserialize)]` wire structs instead of traversing an untyped JSON value; tool-call arguments remain an opaque value and the heterogeneous SSE streaming frames stay dynamic. Citation and grounding evidence is extracted from each provider's reply into the canonical response.

### Removed

- The `systemprompt-ai-wire` crate, the gateway's duplicated per-protocol outbound modules, and the standalone gateway model catalog (`profile/gateway/catalog.rs`).

## [0.13.1] - 2026-06-01

### Changed

- Plugin bundles are generated from the plugin spec instead of served as a pre-built directory tree. `systemprompt-marketplace` now owns the bundle contract: `bundle::build_plugin_bundle` assembles `.claude-plugin/plugin.json`, `skills/<n>/SKILL.md`, `agents/<n>.md`, `.mcp.json`, and scripts from a `PluginConfig` and the resolved catalogue, and both the manifest (`load_plugins`) and the plugin-file byte route build from that one source so their hashes and bytes cannot drift. A spec whose references resolve to no content is skipped rather than emitting an empty, malformed plugin entry, and the per-request `config.yaml` denylists on the manifest and serving paths are removed. `PluginManifest` and the bundle well-formedness predicate move to `systemprompt-models::bridge::plugin_bundle` as the single definition shared with the bridge and CLI.
- `MarketplaceService` resolves the active marketplace solely from `settings.default_marketplace_id` (or the single configured marketplace); the implicit `"default"`-id fallback is removed and `resolve_default` / `active` share one selector.
- `admin setup` generates a complete, bootable profile: the wizard emits a gateway catalog (providers, models, and routes for the AI keys supplied), governance and authz sections, and the gateway-required `hook` resource audience, rather than an empty shell. Env-driven and cloud bootstrap seed the same audiences through `default_resource_audiences()`, and a new `--force` flag overwrites existing profile, catalog, and secrets files.
- Remove the unused `openai_chat_completions::render` module.

### Added

- The gateway gains an optional `default_provider`. When set, a model not matched by any explicit `route` is forwarded to that provider through a synthesized catch-all route instead of being denied, so the gateway is no longer a closed catalog allowlist. `GatewayConfigSpec` / `GatewayConfig` carry `default_provider: Option<ProviderId>`; `GatewayConfig::resolve_route` replaces `find_route` at the dispatch sites and returns the explicit match or the synthesized route as a `Cow<GatewayRoute>`; `is_model_exposed` reports every model as exposed while a default provider is configured; and profile validation rejects a `default_provider` absent from the catalog (`GatewayProfileError::DefaultProviderNotInCatalog`). `admin setup` gains a `--default-provider` flag (and interactive selection) to designate it.
- `admin config` subcommands edit a profile's sections in place — `gateway` (enable state, catalog source, routes), `governance` (authz hook mode), `security` (resource audiences), `secret` (provider credentials), and `catalog` (providers and models) — each validating the result before writing it back.

## [0.13.0] - 2026-05-29

### Breaking

- The marketplace domain is consolidated into the `systemprompt-marketplace` crate. Resolution, validation, candidate assembly, the disk catalog loaders, and Ed25519 manifest signing move out of `entry/api` into `MarketplaceService`, `ManifestService`, `catalog`, and `scope`; the gateway `bridge_manifest` handler and the `/marketplace*` routes are now thin wiring over the domain service. `MarketplaceCandidate` gains `marketplace_id: Option<MarketplaceId>` and `access: Option<MarketplaceAccess>` — external `MarketplaceFilter` implementations that construct or destructure it by field must account for them (a 5-arg `new(...)` and a `with_marketplace(...)` builder are provided). The signed-manifest wire format and canonical signing bytes are unchanged.
- `authz::resolve` / `ResolveInput` gain a `parents: &[ResolveParent]` slice for parent-entity inheritance. Callers constructing `ResolveInput` must supply `parents` — `&[]` preserves the prior single-entity behaviour.
- `AppContext` groups its handles into four cohesive planes — `DataPlane`, `ConfigPlane`, `Plugins`, `Subsystems` (newly exported from `systemprompt-runtime`) — and the flat `AppContextParts` struct is removed. `AppContext::from_parts` now takes the four planes instead of one parts struct. All accessor methods (`config()`, `db_pool()`, `mcp_registry()`, …) are unchanged, so only embedders or tests that build a context directly are affected.
- Filesystem path-existence validation moves out of `systemprompt-models` into `systemprompt_config::path_validation`. `systemprompt_models::config` no longer re-exports `validate_profile_paths`, `validate_required_path`, `validate_optional_path`, or `format_path_errors` (the unused `validate_required_optional_path` is dropped); model-layer profile validation is now pure, with no filesystem access. `validate_postgres_url` stays in `systemprompt-models`.
- `MarketplaceConfig.mcp_servers` is now `PluginComponentRef { source, include, exclude }` instead of a flat `Vec<String>`. Tenants must rewrite YAML from `mcp_servers: [a, b]` to `mcp_servers: { source: explicit, include: [a, b], exclude: [] }`. The flat-list form is rejected at config-load time with a deserialiser error. Validation now resolves `mcp_servers.include` ids against the top-level `services.mcp_servers` catalogue at load time, matching the existing `skills` / `agents` / `plugins` shape on `MarketplaceConfig`.
- All remaining entity-id reference lists across the services config now use `PluginComponentRef` for shape uniformity: `PluginConfig.mcp_servers`, `PluginConfig.content_sources`, `SkillConfig.mcp_servers`, `SkillConfig.assigned_agents`, `DiskAgentConfig.mcp_servers`, `DiskAgentConfig.skills`, `AgentMetadataConfig.mcp_servers`, `AgentMetadataConfig.skills`, `bridge::manifest::AgentEntry.mcp_servers`, `bridge::manifest::AgentEntry.skills`, and `AgentRuntimeInfo.{skills,mcp_servers}`. Authoring YAML must move from flat lists (`mcp_servers: [a, b]`) to the object form (`mcp_servers: { include: [a, b] }`). The deserialiser rejects flat-list inputs with a "expected struct, found sequence" error. `PluginComponentRef` now derives `PartialEq`/`Eq` so it can appear inside `#[derive(PartialEq)]` runtime info structs. `AgentInfo::with_mcp_servers` and `AgentRegistry::get_mcp_servers` callers must thread the `.include` list explicitly when projecting back to `Vec<String>`.

### Security

- Agent task and artifact routes (`GET /api/v1/agent/...`) now verify the caller owns the parent context before returning rows. `TaskRepository::validate_task_ownership`, `ArtifactRepository::validate_artifact_ownership`, and the context-ownership check join through `user_contexts.user_id`; a mismatch is rejected rather than disclosing another principal's tasks or artifacts.
- The authz audit row's `session_id` column records the attested `SessionId` from the gateway enforcement path instead of the trace id. `AuthzRequest` now carries `session_id: Option<SessionId>`; non-session enforcement sites (server-attach RBAC, MCP middleware) record none.

### Changed

- The gateway credential extractor prefers the `Authorization` header over `x-api-key` when both are present (previously `x-api-key` won).
- `ValidatedHookClaims.plugin_id` / `.subject` are now `PluginId` / `UserId`, and the gateway capture/parse/stream-tap path and audit sink carry `AiToolCallId` instead of `String`. The default session-cookie name is centralised on `CookieExtractor::DEFAULT_COOKIE_NAME`.
- Non-OAuth API routes return a new entry-local `ApiHttpError` (`entry/api/src/error/`); the domain-error-to-HTTP-status mapping lives once in its `From` impls rather than at each call site.
- `${VAR}` / `${VAR:-default}` interpolation is consolidated into a single `systemprompt_models::env` primitive (`read_env_optional`, `interpolate`, `contains_placeholder`). The profile loader and the services config layer share one regex and one unresolved-placeholder rule, so the syntax cannot drift between surfaces.
- See `bin/bridge/CHANGELOG.md` 0.9.5: managed MCP servers are registered with Cowork through the bridge loopback proxy with an `Authorization` header instead of `oauth: true`.
- JWT validation is consolidated onto a single RS256 decode primitive, `decode_rs256_claims` (`infra/security/src/jwt/validate.rs`). Request-context middleware, session validation, hook-token validation, and the OAuth / MCP / agent domains all route through it, so the `kid` lookup, RS256 enforcement, and the `exp`/`nbf`/issuer/audience policy live in one place behind a `ValidationPolicy` knob and cannot drift apart. Federated subject-token verification (token-exchange) deliberately remains a separate path — it resolves keys from an external issuer's JWKS rather than this deployment's signing authority.
- JTI revocation moved out of the standalone `entry/api` middleware and into the JWT context extractor as `JtiRevocationChecker` (`middleware/jwt/revocation.rs`). It now runs as the final stateful check — after a token's claims, its backing user, and the session row have all validated — caches negative results for a single-map-lookup hot path, and fails closed: a revocation-store error rejects the request rather than admitting an unverifiable token.
- Server-lifecycle shutdown is extracted into `entry/api/src/services/server/shutdown.rs`: one Ctrl-C / SIGTERM handler with a bounded child-process grace window, wired to scheduler and process-cleanup teardown.
- Rate limiting and request throttling shed dead code paths in the analytics throttle service and the rate-limit middleware; `RouterExt::with_auth` remains the single mount point that requires an `AuthzPolicy`.
- `MarketplaceConfig` gains a declarative `access` block (`default_included`, `roles`, an opaque dotted-namespace `attributes` bag, `justification`). `roles` drive the core RBAC check; `attributes` are forwarded verbatim to extension authz hooks and never interpreted by core, mirroring `JwtClaims.attributes`. Existing marketplace YAML without an `access` block is unaffected.
- Marketplace assignment is declarative and cascades. The access-control sync ingests each marketplace's `access.roles` into `access_control_rules` as `entity_type='marketplace'` rows, and the resolver's parent-entity inheritance lets that grant cascade to the marketplace's member skills, agents, and MCP servers unless a more specific rule applies (deny-overrides). A grant authored once on the marketplace YAML propagates to every member it bundles.
- Profile loading no longer writes back to disk. Gateway route-id backfill is applied in memory and ids are synthesised deterministically from `(pattern, provider)`, so they are stable across loads — removing both a concurrent-invocation write race and the risk of baking interpolated `${VAR}` values into the source profile. `AppContextBuilder::build` now initialises config itself (`try_init_config`), so it no longer depends on a prior bootstrap step having installed the global config.
- The `ServicesConfig` → `Vec<ServiceConfig>` projection consumed by service-state verification lives once as `ServiceConfig::list_from_manifest` (`systemprompt-scheduler`), beside the type, rather than inlined in the CLI dispatch path.
- `POST /api/v1/core/oauth/register` applies the RFC 7591 §2 server defaults when a client omits `grant_types` or `response_types`: missing or empty `grant_types` is treated as `["authorization_code"]`, missing or empty `response_types` as `["code"]`. Previously the handler rejected such payloads with `invalid_client_metadata`, breaking minimal dynamic-client-registration requests from spec-compliant MCP clients (Cowork, Claude Code DCR, MCP Inspector). The persisted client and the response body now both echo the same defaulted arrays — a client wanting `refresh_token` must list it explicitly.
- `bridge_manifest::manifest()` now scopes the manifest's skills, agents, mcp_servers, and plugins to the active marketplace's `MarketplaceConfig.<entity>.include` lists before RBAC filtering. `MarketplaceConfig` was previously parsed but unused at manifest time — the `discover_marketplaces()` step landed the config in `ServicesConfig` and stopped there. Empty `include:` preserves the global-list fallback for backwards compatibility, so deployments without a marketplace authored continue to see the unscoped catalogue. Active-marketplace resolution: zero marketplaces → no scoping; one → that one; many → pick any single entry and emit a `tracing::warn!` (a profile-level selector is a follow-up; intentional fail-open for this release). All four catalogues (`skills`, `agents`, `plugins`, `mcp_servers`) are now uniformly authored as `PluginComponentRef` and scoped via the same helper.
- `mcp::Deployment.endpoint` is now `Option<String>` and, for `internal` servers, must be a relative path (e.g. `/api/v1/mcp/<name>/mcp`) or omitted. Absolute URLs are rejected at config-load time with a validator error naming the offending server. The gateway derives the public MCP URL from `server.api_external_url + /api/v1/mcp/<name>/mcp`, so the API external URL is the single source of truth for managed MCP server URLs. `external` MCP servers continue to accept absolute upstream URLs. Bridge manifest URL synthesis (`routes::gateway::bridge_data::load_managed_mcp_servers`) collapses to: relative `endpoint:` (or absent) → `api_external_url + endpoint-or-default`; absolute `endpoint:` is preserved verbatim and only reachable for `external` servers. Downstream `services/mcp/*.yaml` files with `type: internal` must drop the `endpoint:` line; mixed `api_external_url`/`endpoint:` host divergence (e.g. `localhost` vs `127.0.0.1`) previously broke RFC 8707 resource-indicator byte-equality on OAuth challenges.
- `/api/v1/mcp/*` 401 responses omit `error=` from `WWW-Authenticate: Bearer` when no `Authorization` header was sent, per RFC 6750 §3. Bad-credentials responses keep the `error="invalid_token"` form. Spec-compliant MCP clients (Cowork, Claude Code) rely on the no-error variant to start their OAuth discovery handshake.
- See `bin/bridge/CHANGELOG.md` 0.9.4: `deploymentOrganizationUuid` policy key is no longer written; the `cowork enable` doctor check replaces the `cowork marketplace` check; the cowork-plugins integration adapter is consolidated around `emit`/`upsert` with the legacy `marketplace.rs` / `registry.rs` surfaces removed.
- The route-mount context middleware is now four typed sibling layers — `PublicContextMiddleware`, `UserOnlyContextMiddleware`, `A2AContextMiddleware`, `McpContextMiddleware` — instead of a single `ContextMiddleware<E>` with four named constructors that branched internally on a `ContextRequirement` enum. Each flavour's caller-admission contract (Anon admission, session-context fallback, body-rebuild) is now expressed at the type level, so mounting a route under the wrong flavour is a type error rather than a runtime behaviour nobody re-verified.
- `TaskContextInfo.user_id` is now `Option<UserId>`, exposing the database's existing NULL semantics rather than masking them with a sentinel string.
- `ImageGenerationRequest.user_id` is now non-optional. Callers that cannot supply a `UserId` were never authorised to generate images.
- Bridge `marketplace.json` shape matches the current Cowork (Claude 1.5354) reader: top-level `$schema`, `description`, `metadata { description, version, pluginRoot }`, `owner`, and per-plugin `author` / `category` fields; `plugins[].source` is now a plain string path.
- Bridge `known_marketplaces.json` and `installed_plugins.json` carry `installLocation`, `installPath`, `scope`, and `lastUpdated` fields. Foreign sibling entries continue to be preserved verbatim.
- `RequestContext::is_system` and `RequestContext::is_anonymous` are now `const fn`, callable in const contexts.
- `generate_client_tokens` returns a typed `ClientCredentialsError` enum (ClientNotFound, OwnerNotFound, OwnerInactive, OwnerIdMalformed, InvalidScope, InvalidAudience, HookScopeRequiresHookAudience, UserProviderUnavailable, SessionCreate, JwtSign, ConfigUnavailable) instead of `anyhow::Error`. The route handler maps each variant onto the right RFC 6749 §5.2 status — 4xx for recoverable client mistakes, 5xx only for genuine server faults.
- Skill catalog refactor (Phase A): A2A `card.skills` is now derived at serve time by joining `agent.metadata.skills` against the on-disk `services/skills/` catalog. Authored `card.skills` in agent YAML is deprecated, tolerated for backwards compatibility (`#[serde(default, skip_serializing)]` on `AgentCardConfig::skills`), and emits a `tracing::warn!` at config-load time when non-empty. The bridge marketplace's `skills[]` list is locked to `services/skills/<id>/config.yaml` as the single source of truth, and `AgentEntry.skills` mirrors `metadata.skills` only — phantom `card.skills` ids no longer leak into the manifest. Validation no longer requires `card.skills[].id` to resolve on disk; only `metadata.skills` ids are checked. Downstream repos can strip `card.skills:` arrays from agent YAML in a follow-up commit without breaking deserialisation.

### Added

- `SyncError::GatewayUnauthorized { endpoint, status }` represents gateway 401/403 from `/manifest` and `/pubkey` as a distinct error with exit code 10 and an actionable "run `systemprompt-bridge login <sp-live-...>`" message. The GUI surfaces it via the new `sync-gateway-unauthorized` Fluent string.
- `bridge doctor` command groups the bridge-side self-checks (paths, gateway, credentials, loopback secret, pinned pubkey) into a single one-line-per-check diagnostic surface.
- Services-config loader auto-discovers `<services>/skills/<id>/config.yaml` and `<services>/plugins/<id>/config.yaml` and inserts them into `ServicesConfig.skills.skills` / `ServicesConfig.plugins.plugins` at load time. Marketplace / plugin `skills.include` and `mcp_servers.include` references resolve against the on-disk catalogue without each tenant duplicating every skill or plugin id under `services/config/config.yaml`.
- `RequestBaseUrl` axum extractor (`crates/entry/api/src/services/request_base_url.rs`) resolves the gateway's self-identity from the incoming `Host` header against an allowlist seeded from `api_external_url`. OAuth discovery (`/.well-known/oauth-authorization-server`, `/.well-known/oauth-protected-resource`, and `/.well-known/oauth-protected-resource/*`) and the `/oauth/authorize` self-origin carve-out now use it so an RFC 9728 gateway answering on both `127.0.0.1` and `localhost` echoes whichever host the client actually dialled. Loopback / configured-host mismatches still fall back to `api_external_url`; unknown / injected hosts never leak into the response.
- `SelfOrigins` (`crates/entry/api/src/routes/oauth/endpoints/authorize/validation/mod.rs`) carries both the configured `api_external_url` origin and the request-derived origin into the `resource` parameter check on `/oauth/authorize`, so a client constructing the RFC 8707 resource indicator from the discovery response it just received passes the self-origin carve-out under dual-self-identity. Unrelated hosts continue to fall through to the stricter SSRF guard.
- Unit-test coverage expanded across `domain/agent` (a2a_server processing helpers), `domain/oauth` (bridge/jwt/providers/validation modules), `shared/models` (a2a artifact + task metadata, bridge ids), `domain/ai` (gemini/openai image provider HTTP, resilient provider), `app/scheduler` (process_cleanup), and `entry/api/oauth` (dual-self-identity resource validation).

### Removed

- The standalone JTI-revocation middleware module (`entry/api/.../middleware/jti_revocation.rs`) is deleted; the checker now lives in the JWT context extractor.
- `ProfileError::WriteFile` and `ConfigError::{Regex, MissingCaptureGroup}` are deleted with the profile-loader disk write and the standalone services-config variable resolver they backed.
- `ContextMiddleware`, `ContextMiddleware::{public, user_only, full, mcp}`, and the `ContextRequirement` enum are deleted. Callers construct the new sibling middlewares directly.
- The `ContextExtractor::extract_user_only` method is folded into `extract_from_headers`. The single implementor (`JwtContextExtractor`) had identical bodies for both.
- `systemprompt_identifiers::bootstrap::{anonymous, bot, unknown, default, empty_sentinel}` are deleted, along with `UserId::{anonymous, system, bootstrap, is_anonymous, is_system}`. `UserId` values must originate from a row in the `users` table; the middleware persists one before constructing a request context.
- `AiRequestRecord::minimal_fallback` is deleted. Construction failures propagate to the caller, which logs and skips persistence rather than writing a record with a fabricated `user_id`.
- `ExecutionMetadata::default()` is deleted (no production callers).
- `AuthValidationService::validate_request` no longer takes an `AuthMode`; the `AuthMode` enum, the anonymous-context fallback, and `AgentOAuthState::auth_mode` are deleted. The A2A server middleware consequently requires a Bearer token; unauthenticated A2A traffic returns 401.

### Fixed

- `/api/v1/mcp/*` answers unauthenticated requests with the proxy handler's RFC 9728 `WWW-Authenticate: Bearer resource_metadata="…"` 401 challenge again. v0.11.0 wrapped the MCP route in a coarse `AuthzPolicy::restricted_to([User, Admin, Mcp, Service])` gate above the proxy; the proxy already enforces auth and emits the spec-compliant challenge, so the extra gate only collapsed the response to a generic 403 and broke MCP clients (Cowork, Claude Code) that only start their OAuth discovery on a 401.
- Unauthenticated or malformed-bearer requests to `/api/v1/mcp/<unknown>/…` receive the same RFC 9728 401 challenge instead of `404 Service not found`. The proxy engine intercepts `ServiceNotFound` on the MCP branch and promotes it to the existing OAuth challenge when the request is not properly authenticated; authenticated callers continue to receive 404 for a genuinely unknown service. Required so spec-compliant MCP clients can begin OAuth discovery against any `/api/v1/mcp/*` path.
- `services::proxy::auth::OAuthChallengeBuilder` builds the `WWW-Authenticate: Bearer resource_metadata="…"` URL from the incoming request's `Host` header through the same `RequestBaseUrl` resolver the `.well-known/oauth-protected-resource` body uses. Without this the discovery body advertised a `127.0.0.1` resource URL while the 401 challenge still pointed at `api_external_url` (`localhost`), and spec-compliant MCP clients failed to round-trip the RFC 8707 resource indicator. Host-header injection is bounded by the same allowlist (configured host plus loopback aliases when applicable); non-allowlisted hosts fall back to `api_external_url`.
- `SessionMiddleware`'s skip-tracked and bot-classified branches persist a real anonymous users row via `UserProvider::create_anonymous` before constructing `Actor`. Previously these paths returned a sentinel `user_id` (`"anonymous"` / `"bot"`) that violated the documented `Actor` invariant and caused `POST /oauth/register` to fail with a foreign-key violation on freshly-migrated databases.
- Migration `010_backfill_oauth_client_owner_fk.sql` removes orphan `oauth_clients` rows and installs the `oauth_clients_owner_user_id_fkey` foreign key on databases where migration 004's `ADD COLUMN IF NOT EXISTS` silently skipped the constraint. Fresh installs are unaffected; legacy installs upgrade in place.
- Bridge cache and marketplace path joins sanitise version strings before writing to the filesystem; RFC3339-shaped versions containing `:` no longer trip Windows ERROR_INVALID_NAME during `bridge sync`.
- Bridge `sync` propagates per-host emit failures into `SyncSummary::host_failures` and the one-line summary now reads `sync PARTIAL (…) — N host(s) failed: …`, so a silently half-published marketplace surfaces in the GUI Activity panel instead of being reported as `sync ok`.
- `client_credentials` grant rejections log the underlying validation error via `tracing::warn!` (client_id + cause) without changing the opaque `invalid_client` response surface, so operators can distinguish "client not found" from "secret mismatch".
- `is_port_in_use` (MCP) is bounded by a 1-second `TcpStream::connect_timeout`. A SYN_SENT hang on the loopback probe (WSL2 / firewall pathologies) previously blocked the MCP startup worker indefinitely with no log line; refused / timed-out / errored connects each map to "port free" with a distinct tracing line.

## [0.12.1] - 2026-05-27

### Fixed

- `systemprompt_api::services::server::metrics::install_recorder` now caches the `PrometheusHandle` in a process-wide `OnceLock`. Repeated calls in the same process (e.g. multiple `setup_api_server` calls in one test binary) return a clone of the existing handle instead of erroring with "attempted to set a recorder after the metrics system was already initialized".
- `POST /oauth/register` accepts unauthenticated requests, per RFC 7591 §3. Newly registered clients are owned by the requesting anonymous session's user. The RFC 7592 management routes at `/oauth/register/{client_id}` continue to require authentication.
- `GET /oauth/webauthn/complete` receives the one-time `auth_token` minted by `/oauth/webauthn/auth/finish` and `/oauth/webauthn/register/finish`. The bundled WebAuthn template forwards the token on both flows, and the registration finish handler now mints one alongside the authentication finish handler.
- `/oauth/authorize` echoes the client's `state` parameter back to the registered `redirect_uri` verbatim, per RFC 6749 §10.12. Server-side state binding remains active for the integrated admin-login flow whose state is a same-origin return path.

## [0.12.0] - 2026-05-27

### Breaking

- **`JwtClaims.department` and `AuthzRequest.department` removed; replaced by `attributes: BTreeMap<String, serde_json::Value>`.** The single tenant-axis string baked one deployment shape into core. Token issuers populate the bag with namespaced keys (`acme.desk`, `boeing.clearance`); extension hooks read `req.attributes.get("your.key")`. `AuthenticatedUser` loses `department` / `with_department` / `department()` and gains `attributes` / `with_attributes` / `attributes()`. `SessionParams.department: Option<String>` is replaced by `attributes: BTreeMap<...>`. `JwtUserContext` carries `attributes` so the gateway path forwards them onto every `AuthzRequest`. **Every JWT issued by a pre-0.12 build is incompatible with this release**: the claims schema has changed, so all outstanding access tokens, refresh tokens, and sessions must be re-issued — rotate signing keys or wait out existing token lifetimes before upgrading.
- **`AuthzContext` enum replaced with `{ kind: Cow<'static, str>, payload: serde_json::Value }`.** Core mints three kinds — `"none"`, `"gateway.invocation"` (`{ "model": ... }`), `"mcp.tool_call"` (`{ "tool": ... }`) — via the new `AuthzContext::none()` / `gateway_invocation(&ModelId)` / `mcp_tool_call(&McpToolName)` constructors. Tenants add their own kinds with `AuthzContext::extension(kind, payload)`. Typed accessors `gateway_invocation_model()` / `mcp_tool_call_tool()` return `None` on kind mismatch. Pattern matches on the old enum variants no longer compile — switch to constructor calls and `kind` checks.
- **`RuleType::Department`, `DenyReason::DepartmentDeny`, and `MatchedBy::DepartmentAllow` removed from the resolver.** `ResolveInput` drops its `department` field. `access_control_rules.rule_type` is narrowed to `('role','user')` via migration `008_drop_department_acl.sql`, which also deletes any existing `rule_type='department'` rows. Department-as-rule moves out of core — tenants that need attribute-based rules write a hook (or compose `RuleBasedHook` with their own).
- **`AccessControlConfig.departments` and `RuleEntry.departments` removed.** The exported `DepartmentEntry` type is gone. YAML files that declared top-level `departments:` or per-rule `departments:` arrays must drop them before upgrading; `deny_unknown_fields` rejects either form. `IngestReport.departments_declared` is gone. `AccessControlRepository::list_role_department_rules_for_export` is renamed `list_role_rules_for_export`.

### Added

- **`RuleBasedHook` — the core RBAC resolver promoted to a first-class `AuthzDecisionHook`.** Wraps the sync `authz::resolver::resolve` so extension hooks can compose it explicitly via `CompositeAuthzHook`. Bootstrap composes `[RuleBasedHook, ...extensions]` automatically when a DB pool is available; `mode: webhook` composes `[RuleBasedHook, WebhookHook]`. The implicit "resolver runs before the hook" flow is gone — every decision is a hook now.
- **`AuthzSource::RuleBased` audit-source variant** (`policy = "authz_rule_based"`) so `RuleBasedHook` decisions stay observable alongside webhook and extension rows in `governance_decisions`.
- **`AuthzContext::extension(kind, payload)` constructor and `AuthzContext::{NONE_KIND, GATEWAY_INVOCATION_KIND, MCP_TOOL_CALL_KIND}` const literals** for tenants minting their own enforcement-site kinds.

## [0.11.3] - 2026-05-26

### Breaking

- **Gateway profile section split into on-disk spec and runtime types.** `Profile.gateway` is now `Option<GatewayState>` (enum `Spec(GatewayConfigSpec) | Resolved(GatewayConfig)`); runtime read paths call `GatewayState::resolved() -> Option<&GatewayConfig>`. The on-disk `gateway.catalog_path: <path>` field is removed — write `gateway.catalog: { path: "..." }` for the file-backed form or `gateway.catalog: { providers: [...], models: [...] }` for the inline form (`deny_unknown_fields` rejects the old key). New public types `GatewayConfigSpec`, `GatewayCatalogSource`, `GatewayState` are exported from `systemprompt_models::profile`; the runtime `GatewayConfig` loses `Deserialize` / `schemars::JsonSchema` and is constructed only via `GatewayConfigSpec::resolve(profile_dir)`. Mirrors the existing `GatewayPolicySpec` / `GatewayPolicyConfig` pattern in the AI domain.
- **`ServicesConfig.content` field removed; `services/content/config.yaml` is loaded directly.** `crates/shared/models/src/services/content.rs` and the `pub mod content` declaration are gone; the loader aggregator no longer wraps the file under a `content:` key. `load_content_config` now reads the bare `ContentConfigRaw` shape from `paths.content_config()` directly. Deployments that listed `../content/config.yaml` under `services/config/config.yaml`'s `includes:` must keep it there — the aggregator will now reject the bare `content_sources:` key with a clear "unknown field" error rather than silently coercing operators into either wrapping the file or dropping the include.
- **`JwtUserContext` narrowed to a single `role: Permission`; `roles: Vec<String>` and `department` removed.** Gateway authz call sites that need RBAC roles must look them up from `UserRepository::find_by_id` (the pattern `bridge_whoami` already uses); department is no longer carried on the JWT context.
- **`AuthzMode::Extension` added; the binary supplies the authz hook at bootstrap via `AppContextBuilder::with_authz_hook(...)`.** Bootstrap errors if `extension` mode is selected and no hook is registered. The pre-existing `webhook` / `disabled` / `unrestricted` modes are unchanged.
- **`AppContextBuilder::with_authz_hook` is now generic over `H: AuthzDecisionHook + 'static`.** Callers pass an owned hook value; the builder wraps it in `Arc` internally. Callers that already hold an `Arc<dyn AuthzDecisionHook>` (e.g. a `CompositeAuthzHook`) use the new `with_shared_authz_hook(SharedAuthzHook)` method instead.
- **`SharedAuthzHook` re-exported from `systemprompt_security::authz::hook`** (previously `authz::runtime`). The `authz` facade re-export is unchanged, so `systemprompt_security::authz::SharedAuthzHook` continues to resolve.

### Added

- **`register_authz_hook!` — inventory-based authz hook registration for binaries that do not own the builder site.** Extensions linked into a CLI binary that runs through `systemprompt::cli::run()` can register an `AuthzHookRegistration` factory; `build_authz_hook` discovers it after the database pool exists and threads in an `AuthzHookContext { pool, sink }`. Builder-supplied hooks (`AppContextBuilder::with_authz_hook`) continue to take precedence.
- **`with_shared_authz_hook(SharedAuthzHook)` builder method** for callers that already hold a pre-composed `Arc<dyn AuthzDecisionHook>` (e.g. a `CompositeAuthzHook` shared across consumers).
- **`AuthzSource::ExtensionHook` audit-source variant** (`policy = "authz_extension_hook"`). Extension hooks record through `AuthzAuditSink::record(_, _, AuthzSource::ExtensionHook)` so audit consumers can filter every decision the extension path produced; variant attribution stays on the `Deny` decision's `policy: String`.
- **`DenyReason::PolicyViolation { policy, detail }` variant** for extension-issued denies. Outer `AuthzDecision::Deny.policy` carries the policy identifier (e.g. `"abac.itar"`); `detail: Cow<'static, str>` is the human-readable reason.
- **`build_gateway_authz_request` (`systemprompt_api::routes::gateway::messages`) and `build_mcp_authz_request` (`systemprompt_mcp::middleware::rbac`)** — pure helpers that assemble the `AuthzRequest` from claims/principal inputs, exposed so the JWT-claims forwarding contract can be unit-tested without standing up a hook or RBAC stack.
- **`SYSTEMPROMPT_TRUSTED_HTTP_HOSTS` — operator opt-in for sealed-network http endpoints.** New public API on `systemprompt-models`: `validate_outbound_url_with_trust(url, trusted_http_hosts)` and the helper `trusted_http_hosts_from_env()`. Hosts on the comma-separated allowlist pass the http scheme gate and bypass the IP block for that hostname only; every other host continues to hit the strict default (loopback-only http, RFC1918/metadata block enforced). `validate_outbound_url(..)` is unchanged. The gateway profile validator reads the env automatically — empty when unset, so existing deployments keep the prior behaviour. The agent-process and MCP-process spawners now forward the variable into child environments after `env_clear`, so subprocess catalog re-validation sees the same allowlist as the parent.

### Fixed

- **`systemprompt-api` now builds on macOS.** `get_disk_usage()` in `services/server/health.rs` previously fed `nix::sys::statvfs` block counts straight into `saturating_mul`. Those fields alias to `libc::fsblkcnt_t`, which is `u64` on Linux but `u32` on Darwin, so the arithmetic type-checked only on Linux. Every field is now widened to `u64` before multiplication, making the function portable across both targets.
- **`JwtContextExtractor::decode_for_gateway` no longer discards its own `validate(...)` return value with `let _ = ...await?`.** The bound was a no-op (`validate` returns `()` on success and the `?` already propagated the error) but tripped the `let _ = <fallible>` ban. The call is now a plain statement.
- **`AuthzBootstrapError::NoGovernanceButExtensionHook` replaces the misleading `ExtensionHookButWrongMode { mode: "disabled" }` reported when no `governance.authz` block was present.** The previous variant claimed a `mode` that never existed; the new variant points the operator at the actual misconfiguration (either set `governance.authz.hook.mode = extension` or drop the `with_authz_hook` call).

### Changed

- **Eleven source files split below the 300-line cap.** Affects no public API: `authz/types`, `authz/repository`, `keys/jwks_client`, `profile/gateway`, `oauth/refresh_token`, `scheduling`, `entry/cli admin access-control`, `entry/cli infrastructure db schema`, and the three a2a-server agent files (`streaming/handlers/completion`, `processing/message/message_handler`, `processing/message/stream_processor/processing`) are each promoted to a `mod.rs` + cohesive submodules. Re-exports preserved so downstream `use` paths are unchanged. `just file-size` is now empty.

## [0.11.2] - 2026-05-25

### Breaking

- **`access_control_rules` split into `access_control_entities` + `access_control_rules`.** Migration `007_split_acl_entities.sql` creates the new catalog table (`entity_type`, `entity_id`, `default_included`, `source`, `created_at`, `updated_at`, PK on `(entity_type, entity_id)`), promotes every pre-split sentinel row (`rule_type='role' AND rule_value='__default__'`) into it with `source='bootstrap:default_promoted'`, back-fills derived entities for orphan grants with `source='bootstrap:rule_derived'`, deletes the sentinel rows, drops `access_control_rules.default_included` and `idx_acl_default`, then adds a composite FK on `access_control_rules(entity_type, entity_id)` referencing the catalog.
- **`AccessRule::default_included` field removed.** The flag now lives on `EntityRow` (new type in `authz::types`). `AccessRule`'s serde wire shape loses the field — every deserialisation site that constructed `AccessRule` literals must drop it; integration tests too.
- **`AccessControlRepository` rewritten to a two-table API.** New: `get_entity(EntityKind, &str) -> AuthzResult<Option<EntityRow>>`, `upsert_entity(EntityKind, &str, default_included: bool, source: &str) -> AuthzResult<()>`, `list_entities(EntityKind) -> AuthzResult<Vec<EntityRow>>`. Removed: `get_default_included`, `set_default_included` — callers must transit through `get_entity` / `upsert_entity` (a `None` lookup now signals `UnknownEntity` to the resolver). `list_rules_for_entity` / `list_rules_bulk` no longer filter the sentinel rule_value — the sentinel scheme is gone.
- **`AccessControlIngestionService` upserts entity rows alongside grants.** Each rule in the YAML config now produces an `access_control_entities` row (`source='ingestion:access_control_config'`, `default_included=false`) before the grant is inserted, so the FK on `access_control_rules` is satisfied. `delete_orphans` no longer needs to preserve the sentinel — it sweeps every `role`/`department` rule.
- **`systemprompt_mcp::MCP_PROTOCOL_VERSION` constant removed.** Use `systemprompt_mcp::mcp_protocol_version() -> String` or `systemprompt_mcp::mcp_protocol_version_str() -> &'static str`. Both resolve to `rmcp::model::ProtocolVersion::LATEST` and track the linked `rmcp` release.
- **`GatewayPolicySpec::allowed_models` and `GatewayPolicySpec::model_allowed` removed.** Model exposure is now owned by the profile's `GatewayCatalog` (see `GatewayConfig::is_model_exposed`). A request whose `model` is not declared in the catalog is rejected with `403` before route resolution — the old policy-allow-list path is gone. Deployments that still set `allowed_models:` in `services/ai/gateway-policies.yaml` MUST remove the field (the spec now uses `deny_unknown_fields`).
- **`GatewayProvider`, `GatewayModel`, and `GatewayRoute` entity-id fields are now typed.** `GatewayProvider.name: ProviderId`, `GatewayProvider.api_key_secret: SecretName`, `GatewayModel.id: ModelId`, `GatewayModel.provider: ProviderId`, `GatewayModel.aliases: Vec<ModelId>`, `GatewayRoute.id: RouteId`, `GatewayRoute.provider: ProviderId`, `GatewayRoute.api_key_secret: SecretName`. `pub fn synthesize_route_id(...) -> RouteId` (was `-> String`). Call sites comparing or stringifying these fields must use `.as_str()`. YAML deserialization is unchanged — `define_id!` derives `Deserialize` transparently.
- **`AuthzRequest` now carries `entity: EntityRef`** instead of the `{entity_type: EntityKind, entity_id: String}` pair. `EntityRef` is a `#[serde(tag = "kind", content = "id")]` tagged union over the eight typed ids (`RouteId`, `ModelId`, `McpServerId`, `PluginId`, `AgentId`, `MarketplaceId`, `SkillId`, `HookId`), so the discriminator and id can no longer drift apart on the `/govern/authz` wire. Webhook handlers must read `req.entity.kind()` and `req.entity.id_str()`. The audit JSON in `governance_decisions.evaluated_rules` keeps the flat `entity_type` / `entity_id` keys — that JSONB is an internal store, not a wire contract.
- **`services/ai/gateway-policies.yaml` back-compat fallback removed.** The gateway-policy loader is now single-path on `services/gateway/policies.yaml`. Deployments that still ship the legacy file MUST move it before upgrading.
- **`GatewayRoute.endpoint` and `GatewayRoute.api_key_secret` removed.** Both fields live exclusively on `GatewayProvider`; the route references its provider by `ProviderId` and resolves endpoint + secret through the catalog. New helper: `GatewayRoute::resolve(&self, providers: &[GatewayProvider]) -> Option<&GatewayProvider>`. YAML migration: delete `endpoint:` and `api_key_secret:` from every `gateway.routes[*]` entry — `deny_unknown_fields` now rejects them. The `RouteEndpointMismatch` validate error is gone (the drift it caught is now structurally impossible). `pub fn synthesize_route_id` signature shrinks from `(pattern, provider, endpoint)` to `(pattern, provider)`; the 6-hex hash tail is recomputed accordingly, so route ids that were previously hash-derived will change shape and operators relying on synthesized ids must rerun `ensure_route_ids` or rewrite the affected `access_control_rules` rows.
- **Authz enforced on `/v1/messages`** before upstream dispatch: the gateway evaluates `EntityRef::GatewayRoute(RouteId)` against the resolved route and denies on `403`; webhook fault defaults to deny via the existing `WebhookHook::fault` path. The decision is audited via `AuthzAuditSink`, correlated by `trace_id`. Previously the gateway dispatch path was unauthenticated.
- **`GatewayConfig::validate()` rejects duplicate route ids.** New variant `GatewayProfileError::DuplicateRouteId { id }` fires when two routes — whether both explicitly authored or one synthesized — collide on `id`. This guards the (vanishingly unlikely) 6-hex synthesis collision and the more common "operator copy-pasted a route block" case.
- **Authz `Decision::Allow` is now a struct variant carrying `matched_by: MatchedBy`.** Callers pattern-matching on `Decision::Allow` (unit) must switch to `Decision::Allow { .. }`; constructors must supply a `MatchedBy` (one of `UserAllow`, `RoleAllow { role }`, `DepartmentAllow { department }`, `DefaultIncluded`, `PolicyAllow { policy_id, detail }`).
- **`Decision::Deny.reason` is now typed `DenyReason`** (was `String`), and `Decision::Deny.justification` is gone — justification is folded into the per-variant `justification: Option<String>` field on `DenyReason::{UserDeny, RoleDeny, DepartmentDeny}`. `AuthzDecision::Deny.reason` is also `DenyReason`. The `#[error]` strings on each variant double as the audit row `reason` column (use `.to_string()`).
- **`AuthzRequest.context` is now typed `AuthzContext`** (was `serde_json::Value`). Variants: `GatewayInvocation { model: ModelId }`, `McpToolCall { tool: McpToolName }`, `None`. Missing/null on the wire deserializes to `None`.
- **`resolver::resolve` signature is now `resolve(input: ResolveInput<'_>) -> Decision`.** `ResolveInput` bundles `{ entity: &EntityRef, rules, user_id, user_roles, department, default_included: Option<bool> }`. `default_included: None` signals "no entity row" → `DenyReason::UnknownEntity`; `Some(false)` → `DenyReason::NotAssigned`.
- **`Decision`-bearing audit code paths must call `.to_string()` on `DenyReason`** to populate the SQL `reason` column — `DenyReason: thiserror::Error + Serialize`, but the column is plain TEXT.

### Added

- **`systemprompt admin access-control lint` CLI subcommand.** Reads `access_control_entities` + `access_control_rules` and reports two failure modes: rules pointing at no catalog row (`UNKNOWN`) and catalog rows with `default_included=false` and zero grants (`UNREACHABLE`). Exits non-zero on findings so it can gate CI.
- **`EntityRow` struct** in `authz::types` — `{ kind: EntityKind, id: String, default_included: bool, source: String }`. Round-trips through serde; re-exported from `authz`.
- **Per-crate test for `EntityRow` serde + AccessRule regression** in `crates/tests/unit/infra/security/authz/src/entity_row.rs`.
- **`GatewayConfig::is_model_exposed(&str) -> bool`** — single dispatch-time gate that consults the profile catalog. Replaces the per-policy `allowed_models` allow-list as the source of truth for "is this model exposed."
- **`GatewayConfig::validate()`** — boot-time cross-check that fails loud on catalog/route drift. Verifies: every route's provider exists in `GatewayCatalog.providers`; every catalog model id and alias is globally unique; every catalog model is reachable by at least one route pattern. New error variants: `RouteProviderNotInCatalog`, `DuplicateModelId`, `UnreachableModel`.
- **`GatewayModel::aliases: Vec<String>`** — alternate model ids that resolve to the same catalog entry for exposure and `/profile` listing (e.g. `claude-opus-4-7[1m]` aliasing `claude-opus-4-7`).
- **`McpToolName` typed id** in `systemprompt_identifiers::mcp`. Non-empty `define_id!` wrapper for MCP protocol tool names.
- **`PolicyId` and `SecretPatternId` typed ids** in `systemprompt_identifiers::policy`. `PolicyId` is the canonical key for governance policies; `SecretPatternId` names secret-scanner patterns.
- **`systemprompt_security::policy` module** — shared types for the tool-use governance plane: `AgentScope { User { user_id } | System }`, `McpToolInput` (the documented inline-`serde_json::Value` boundary for schema-less MCP arguments), `PolicyContext<'a>`, `SecretLocation { kind, path }`, `RateLimitWindow { name, seconds, limit }`, `GovernancePolicy` trait, and `GovernanceChain` (first-deny-wins composer; allow-fallthrough is `MatchedBy::DefaultIncluded`).
- **`DenyReason` tool-use variants**: `SecretLeak`, `ScopeViolation`, `ToolBlocked`, `RateLimitExceeded`, `HookUnavailable`. Lets the template's secret-scan / scope / blocklist / rate-limit chain emit the same `Decision` shape the user→entity resolver does.

### Changed

- **`GATEWAY_POLICIES_FILE` moved from `services/ai/gateway-policies.yaml` to `services/gateway/policies.yaml`.** The loader is single-path on the new location.
- **Workspace lint surface tightened.** Added `clippy::map_err_ignore`, `clippy::str_to_string`, `clippy::undocumented_unsafe_blocks`, `clippy::self_named_module_files`, `clippy::allow_attributes`, `clippy::allow_attributes_without_reason`, `rust::unreachable_pub`, `rust::unused_lifetimes`, and `rust::single_use_lifetimes` at `warn`. The previously-disabled `cognitive-complexity-threshold` and `too-many-lines-threshold` in `clippy.toml` are re-engaged at `30` and `150` respectively (from `999999`). `rustfmt.toml` `edition` now matches the workspace at `"2024"`.
- **Internal visibility narrowed across the workspace.** `unreachable_pub` reduced from 1,146 to 58 warnings: items only used within their crate are now `pub(crate)`. Cross-crate consumers (re-exports through `lib.rs`, the `systemprompt` facade) keep `pub`.
- **`map_err(|_| ...)` callers renamed to `|_e| ...`** across 113 sites outside the identifiers crate, silencing `clippy::map_err_ignore` without changing behaviour. Where the discarded error genuinely belongs in the source chain, the next pass should switch to `map_err(|e| ...with_source(e))` instead of the silenced form.
- **Workspace clippy clean outside `redundant_pub_crate`.** Cleared the remaining ~230 stragglers across the non-test crates: 58 `unreachable_pub` items lowered to `pub(crate)` (with split re-exports where the facade still consumed them publicly), 14 `private_in_public` warnings resolved by promoting clap arg structs and CLI output payloads, 46 `str_to_string` / `ToString::to_string` sites rewritten to `.to_owned()`/`str::to_owned`, 13 `assigning_clones` rewritten to `clone_into`, 81 `allow_attributes_without_reason` / `expect_attributes_without_reason` sites either deleted (where the suppression was stale) or given a substantive reason, 27 unfulfilled `#[expect]` blocks removed, six single-file modules renamed to `mod.rs`, and the final `#[allow]` converted to `#[expect]`.
- **Three long functions split into smaller helpers** so every function fits under the 150-line threshold: `MessageProcessor::handle_message` (extracted `collect_stream_response`, `broadcast_agui_lifecycle`, `persist_or_mark_failed`); `StreamProcessor::process_message_stream` (extracted `run_stream_pipeline`); `handle_complete` (extracted `build_complete_task`, `broadcast_task_success`).
- **Two new `just` recipes:** `just machete` (unused dependencies via `cargo-machete`) and `just hack` (feature-powerset build via `cargo-hack`).
- **`bin/bridge` clippy posture matches the main workspace.** `bin/bridge/Cargo.toml` adopts the workspace `[lints.clippy]` / `[lints.rust]` baseline (deny `clippy::all` + `suspicious`, warn pedantic/nursery/cargo/perf, `unreachable_pub`, `missing_debug_implementations`, `allow_attributes_without_reason`, …). The bridge now builds clean under `cargo clippy --manifest-path bin/bridge/Cargo.toml --all-targets --no-deps -- -D warnings`.
- **`clippy::redundant_pub_crate = "allow"` at the workspace level.** It conflicts with `unreachable_pub` for the repo's deliberately-narrowed `pub(crate)` module hierarchy, and the visibility-cleanup work has already chosen the narrower form.
- **Bridge inline tests extracted.** `#[cfg(test)] mod tests` blocks under `bin/bridge/src/{config/mod.rs, lib.rs}` move to dedicated test crates `crates/tests/unit/bridge/{config, ts-export}/`.

### Fixed

- **MCP protocol-version reporting unified on `rmcp::model::ProtocolVersion::LATEST`** (currently `2025-11-25`). `McpDeploymentProvider::protocol_version` and the `supported_protocols` field of the `systemprompt:mcp-tools` agent extension previously returned a hardcoded `2024-11-05`, while `McpServiceProvider::protocol_version` already used the SDK value.
- **`define_id!` macro no longer emits `&str::to_string()` or unannotated `#[allow]` attributes.** The `From<&str>` impl, the `system`/`bootstrap`/`Default` constructors in `agent`, `session`, `trace`, `profile`, `user`, `client`, and `policy`, and the `db_value` conversions now use `.to_owned()`; the macro's internal `#[allow(clippy::expect_used)]` is replaced with `#[expect(..., reason = "...")]`. Eliminates ~2,700 warnings under the newly enabled `clippy::str_to_string`, `clippy::allow_attributes`, and `clippy::allow_attributes_without_reason` lints.

## [0.11.1] - 2026-05-22

Hardening pass on the 0.11.0 governance-audit path: the `governance_decisions.actor_kind` CHECK is re-aligned with `ActorKind`, the audit insert is typed at the Rust boundary so the next enum/constraint drift fails at `cargo check`, and a Prometheus counter surfaces write failures. The `web validate` and `web sitemap show` CLI commands also recover from two long-standing configuration-shape mismatches that produced false warnings and missed sitemap routes. CLI startup latency on cloud-touching commands is reduced by caching `/api/v1/auth/me` validation on disk, and the interactive session banner is now silent on local profiles at default verbosity. Several CLI ergonomics bugs uncovered by a 293-leaf command sweep are fixed: `plugins mcp call` surfaces the MCP server's real error and exits non-zero on failure, `admin agents task` no longer silently picks up a stale `SYSTEMPROMPT_TOKEN`, `core content show` disambiguates slug lookups, and `web validate` no longer fails the exit code on warnings.

### Breaking

- **`GovernanceDecisionRecord.decision` is now `DecisionTag`** (re-exported from `systemprompt-security::authz`) instead of `&str`. Callers constructing the record from an `AuthzDecision` use `DecisionTag::from(&decision)`; from a `Decision`, use `decision.tag()`. The change makes the column allow-list and the Rust enum drift together at compile time.

### Added

- **`ActorKindTag` in `systemprompt-identifiers`** — discriminant-only view of `ActorKind` with `#[derive(sqlx::Type)]` (behind the `sqlx` feature). Bound by the audit writer as the `actor_kind` column value via `ActorKind::tag()`.
- **`DecisionTag` in `systemprompt-security::authz`** — discriminant-only view of `Decision` / `AuthzDecision` with `#[derive(sqlx::Type)]`, plus `Decision::tag()` and `From<&AuthzDecision> for DecisionTag`.
- **`governance_audit_write_failed_total` Prometheus counter** — incremented inside `insert_governance_decision` whenever the INSERT errors, labelled `{actor_kind, decision, policy}`. Exposed as `pub const AUDIT_WRITE_FAILED_TOTAL` so alert rules can reference the metric by symbol.
- **Schema-drift unit tests** under `crates/tests/unit/infra/security/authz/`: `actor_kind_schema.rs` and `decision_schema.rs` assert every enum variant appears in the table's CHECK allow-list and the most recent migration. The next variant added without a matching schema update fails CI instead of dropping rows in production.

### Fixed

- **`governance_decisions` CHECK constraint extended to every `ActorKind` variant** by migration `005_actor_kind_extend.sql`. The base allow-list `('user', 'job', 'mcp')` did not include `anonymous`, `system`, or `agent`, so every hook write that resolved to `Actor::agent(...)` was rejected by the constraint inside a detached `tokio::spawn` and the failure never reached the caller. Migration is idempotent.
- **`web validate` and `web sitemap show` accept both content-config shapes.** `services/content/config.yaml` is now parsed via `systemprompt_models::content_config::parse_content_config`, which transparently unwraps a top-level `content:` key when present. Previously direct `ContentConfigRaw` deserialisation silently produced an empty `content_sources` map against the wrapped form, so `web validate` warned on every template binding and `web sitemap show` returned zero routes. The runtime landing-page loader in `systemprompt-runtime` uses the same helper.
- **`web validate` resolves logo and favicon paths through the storage root.** Asset existence is now checked against `BrandingConfig.logo.{primary,dark,small}` and `BrandingConfig.favicon`, with each `/files/...` URL resolved under `profile.paths.storage_resolved()` — matching how the HTTP runtime serves them. The previous implementation substring-matched filenames against `services/web/config.yaml` and looked for them in `services/web/assets/logos/`, a directory the runtime never serves from, and reported false errors against any project whose assets live under `storage/files/images/`.
- **`plugins mcp call` propagates the underlying MCP error and exits non-zero on failure.** The transport-layer wrap that flattened every rejection into `error: "Tool execution failed"` with exit code 0 is removed; the rmcp error text is surfaced verbatim, an `is_error=true` `CallToolResult` is treated as a failure path, and the result card is still rendered before the process exits non-zero. Scripted callers can now branch on `$?`.
- **`admin agents task` reuses the active CLI session and 404s before 401.** The `--token` argument no longer reads `SYSTEMPROMPT_TOKEN` from the environment, so a stale env-var token can no longer silently override the session token that the sibling `admin agents message` was already using successfully. Both commands now share a single A2A request helper (`crates/entry/cli/src/commands/admin/agents/client.rs`) and pre-validate agent existence via `ConfigLoader::load`, so a ghost agent returns "Agent not found" instead of the upstream gateway's 401.
- **`core content show <slug>` disambiguates across sources.** A new `ContentRepository::find_sources_by_slug` query enumerates every source that holds the slug for the active locale. Slug lookups with a unique match no longer require `--source`; ambiguous lookups list the candidate source IDs in the error message; missing slugs say so explicitly.
- **`admin agents logs` strips ANSI escapes and `[profile: …]` banner lines from JSON output.** Tracing field values are stripped of CSI sequences at write time in `infra/logging`'s `FieldVisitor`, and `logs_db.rs` additionally applies `console::strip_ansi_codes` and filters out CLI-banner lines on read, so the `logs[]` array now parses as plain text under `jq`.
- **`plugins mcp validate` accepts `--service` as an alias for the positional server name.** Eliminates the doc-drift trap where prior issue-list references to `--service <SERVER>` failed with `unexpected argument`.
- **`admin users role promote|demote` clarifies its scope in `--help`.** A new `long_about` makes explicit that these subcommands only operate on the built-in `admin` role and direct users to `admin users role assign --roles <ROLE>...` for any other role.
- **`infra db query` surfaces the raw Postgres error verbatim.** The custom paraphrase ("Table or relation 'X' does not exist", "SQL syntax error: …") is removed; the Postgres message is shown as-is and the "Did you mean …" hint, when available, is appended on a separate line.
- **`web validate` exit code reflects errors only.** Warnings no longer fail the command — `valid` is now `errors.is_empty()`. The warning report is still rendered. Exit code is 0 with warnings, 1 only when an error is recorded.

### Changed

- **Cloud credential validation is cached for 15 minutes on disk.** `CredentialsBootstrap::init` now records a `last_validated_at` timestamp on `CloudCredentials` after a successful `/api/v1/auth/me` round-trip and skips the call on subsequent invocations within `credentials::VALIDATION_TTL_SECS` (900s) — provided the JWT is not within its 1h expiry warning window. Removes the ~500 ms HTTPS round-trip from back-to-back `systemprompt cloud …` and `systemprompt admin session …` invocations. Existing credential files without the field re-validate once and persist the timestamp.
- **Interactive `[profile: … | session: …]` banner is gated to verbose or non-local profiles.** The session-context banner emitted by `get_or_create_session` now follows the same policy as `profile_banner` (`crates/entry/cli/src/bootstrap.rs`): local profile at default verbosity is silent; `--verbose` or a cloud-target profile shows the line. Removes visual noise from interactive local-profile demos.

### Security

- **MCP schema validator rejects unsafe table identifiers.** `SchemaValidator::validate_columns` charset-checks `schema_def.table` against `^[A-Za-z_][A-Za-z0-9_]{0,63}$` before building the `PRAGMA table_info(...)` query. The previously-`pub` and unused `SchemaValidator::get_table_info` is removed; only the validated path can reach the PRAGMA. SQLite cannot parameter-bind identifiers in PRAGMA / DDL, so charset validation is the only available defence.
- **Default HTTP request-body limit reduced from 100 MiB to 2 MiB.** `apply_global_middleware` in `crates/entry/api/src/services/server/builder.rs` now caps generic request bodies at 2 MiB; routes that intentionally accept large payloads continue to use `DefaultBodyLimit::disable()` per-route. Closes a generic body-flood amplification path.
- **`/api/v1/admin/cli` validates argv before spawning the subprocess.** A new `validate_cli_args` rejects argv with control characters, shell metacharacters (`` ` $ | ; & \n \r``), oversized args (>256 B), too many args (>32), or a first arg that is not a lowercase subcommand token. The endpoint remains admin-gated; the validator is defence-in-depth against argv smuggling reaching any downstream tool that does invoke a shell.
- **CORS origin validation rejects wildcards and non-HTTPS.** `Config::validate_cors_origins` now rejects `*` and accepts only `https://...` origins, with an explicit carve-out for `http://localhost`, `http://127.0.0.1`, and `http://[::1]` for local development.
- **Process cleanup pattern arguments are charset-validated.** `kill_by_pattern` on both the POSIX (`pkill -f`) and Windows (`taskkill /IM *...*`) backends rejects patterns containing anything outside `[A-Za-z0-9_.\-]` (POSIX additionally permits `/`) before invoking the platform command.
- **File upload path components are structurally validated.** `determine_storage_path` rejects filenames containing path separators, `..`, `.`, or NUL bytes, and rejects any relative-path component that is a parent-dir, root-dir, or prefix. Removes the symlink-blind `contains("..")` heuristic in favour of a per-component check.
- **Structured logging redacts known secret-bearing field names.** `FieldVisitor` in `crates/infra/logging/src/layer/visitor.rs` emits `"[REDACTED]"` for fields whose name matches (case-insensitively) any of `password`, `passwd`, `secret`, `token`, `access_token`, `refresh_token`, `id_token`, `authorization`, `auth_token`, `cookie`, `set-cookie`, `api_key`, `apikey`, `client_secret`, `private_key`.
- **WebAuthn pending-state maps are capacity-bounded.** `WebAuthnService::cleanup_expired_states` now caps `reg_states`, `auth_states`, and `verified_auths` at 10 000 entries each (oldest-first eviction) on every sweep, providing a hard backstop against unbounded growth between sweeps. The intentional `webauthn-rs` `danger-allow-state-serialisation` feature is annotated with its operational rationale and compensating control.
- **Cloud credentials bootstrap warning is tagged as an audit event.** When `SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS=1` causes the API validation step to be skipped, the warning is emitted under `target: "security_audit"` so log routing can flag it.
- **`/oauth/logout` returns 500 on Set-Cookie header construction failure.** The previously-swallowed `HeaderValue::from_str` error is now logged at error level and converted to an `OAuthHttpError::server_error` — the cookie-clear must succeed before the client is told its session is over.
- **Webhook outbound HTTP client has bounded timeouts.** `WebhookService::new` now constructs `reqwest::Client` with a 10s overall timeout and a 5s connect timeout; the previous defaults left calls unbounded until the kernel gave up.
- **Webhook delivery rejects loopback and private-network destinations.** `send_webhook` validates the target URL before dispatching: `https` only (with an explicit `http://localhost` / `127.0.0.1` / `[::1]` carve-out), rejecting the AWS metadata host (`169.254.169.254`), `10.0.0.0/8`, `172.16.0.0/12`, and `192.168.0.0/16`. Prevents an operator-misconfigured webhook from exfiltrating cloud-metadata endpoints or pivoting to internal services.
- **`admin keys issue-plugin-token` mints a service principal, not an admin token.** The token is now scoped to `hook:govern` + `hook:track` with `aud=hook`, so it decodes as `user_type=service` and matches what the hook validator (`crates/infra/security/src/auth/hook_token.rs`) already requires. Previously it carried `Permission::Admin` with `aud=api`, so a 365-day "plugin" token was Admin-typed and accepted on the general API surface. The admin check to *mint* the token is unchanged. Re-issue any existing plugin tokens to pick up the narrower scope.
- **`user_type` is re-derived from permissions during JWT validation.** The Axum auth extractor (`crates/entry/api/src/services/middleware/jwt/token.rs`) now derives the caller type from the token's scope via `UserType::from_permissions` and rejects any token whose `user_type` claim disagrees, rather than trusting the claim verbatim. The derivation is centralised in `systemprompt-models` so `AuthenticatedUser::user_type()` and the validator cannot drift; as part of this, the `hook:govern` / `hook:track` scopes now resolve to `service` instead of falling through to `anon`.
- **`POST /content/links/generate` now requires an authenticated user.** The tracking-link generator creates a persistent, redirect-bearing row from a caller-supplied `target_url`; it was reachable anonymously under the public content router, an unauthenticated row-creation and open-redirect surface. The content routes are split so the read-only endpoints (`/query`, content fetch, link-analytics GETs) stay public while `/links/generate` is gated to `AuthzPolicy::user()`.
- **Bridge gateway credentials now persist their session at mint time.** Exchanging a PAT, device certificate, or session code for a bridge JWT writes the backing `user_sessions` row — keyed to the token's `session_id` and populated with the analytics of the exchange request (IP, user-agent, fingerprint) — so the hardened gateway validator admits the token on its first request. Previously the mint path emitted a `session_id` but never created the row, so every freshly issued bridge credential (heartbeat included) was rejected with `401 Invalid JWT token: Session missing or revoked`. The `user_sessions.session_source` CHECK constraint is widened to accept `bridge` and `mcp` (migration `006_user_sessions_source_bridge_mcp.sql`).
- **Bridge JWTs bind to the bridge's own session id.** The gateway's `/v1/messages` validator requires the request's `x-session-id` header to equal the JWT's `session_id`. The bridge holds one stable session id for its process lifetime and sends it on every forwarded request, but the credential exchange minted the JWT with an unrelated freshly generated id, so the header never matched (`401 X-Session-ID does not match authenticated session`). The bridge now sends its stable session id as `x-session-id` on the PAT/session/mTLS exchange, and the server mints the JWT (and its `user_sessions` row) with that id. Session creation is now idempotent, so the bridge's hourly token re-mint re-arms the existing session row instead of failing on the duplicate key. A cached bridge token is also discarded when its session id no longer matches the current process (e.g. after a restart).

## [0.11.0] - 2026-05-20

Multi-replica deployment readiness, the gateway tenancy strip, the Service-JWT sync handshake, mandatory actor attribution on every audit-bearing row, RFC 8693 token-exchange with a published JWKS, federated identities, and a sweep of OAuth and session hardening. AI gateway and bridge-session paths no longer carry a runtime `tenant_id`; sync clients authenticate with a `client_credentials` Service-JWT instead of the legacy shared `SYNC_TOKEN`; events relay across replicas through a Postgres outbox; scheduled jobs claim work behind cross-replica advisory locks; database reads route to a configured read replica. The signing-key plane moves to RSA / RS256 with a published JWKS; HS256 is gone. Pre-1.0 SemVer dictates a minor bump for the `JwtAudience::Cowork → Bridge` rename in `systemprompt-models`.

### Breaking

- **`JwtAudience::Cowork` renamed to `JwtAudience::Bridge`** in `systemprompt-models`; `as_str()` now returns `"bridge"`. Re-issue any persisted JWTs minted under the old audience — tokens with `aud: "cowork"` no longer validate.
- **`CoreError` removed from `systemprompt-models`.** The legacy umbrella enum had no remaining consumers; downstream code must use the per-concern `thiserror` enums (`ConfigError`, `SecretsError`, `RowParseError`, `ProviderError`, `ServiceError`, `ConfigValidationError`, `MetadataError`) re-exported from `errors::*`. `pub use core::CoreError` is gone.
- **`ConfigManager` renamed to `ConfigService`** in `systemprompt-config`. The module file moves from `services/manager.rs` to `services/service.rs`. Update imports (`systemprompt_config::ConfigManager` → `ConfigService`).
- **`OutboxRow::user_id` is now `UserId`** in `systemprompt-events`. The `query_as!` boundary casts `user_id` to `UserId` and downstream consumers (the LISTEN/NOTIFY bridge) consume it as the typed identifier; no more `UserId::new(row.user_id)` coercion at the call site.
- **AI gateway tenancy removed.** The `tenant_id` column is dropped from every `gateway_*` table by migration `003_drop_runtime_tenancy.sql`; repository signatures and request/response types in `systemprompt-ai` no longer carry a tenant parameter. Tenancy continues to live in the cloud deployment plane.
- **`bridge_sessions.tenant_id` removed.** Migration `003_drop_bridge_session_tenant.sql` in `systemprompt-oauth` drops the column; bridge OAuth flows no longer scope to a per-row tenant identifier.
- **Sync routes drop the `SYNC_TOKEN` middleware.** The static shared-secret header is replaced with a `client_credentials` Service-JWT obtained from the `sys_sync` OAuth client. Existing operators must provision the new client and reconfigure sync agents to mint tokens via the OAuth flow; `SYNC_TOKEN` is no longer read.
- **Audit attribution is mandatory.** Every audit-bearing insert (`ai_requests`, `event_outbox`, `governance_decisions`, and the schema-level audit-row contract) now requires an explicit `(actor_kind, actor_id)` pair. The previous default `actor_kind` is dropped; repositories no longer accept a row without one.
- **`McpServerConfig` and `JobConfig` require an owner.** Both types take an owner `UserId` at construction; system-invoked tools resolve their actor through `Actor::from_tool_name`, and the scheduler resolves each job's owner before dispatch.
- **`/oauth/register` now requires authentication.** The endpoint persists the caller as `owner_user_id` on the resulting `oauth_clients` row; anonymous client registration is gone. Migration adds `owner_user_id` to `oauth_clients`.
- **Existing HS256-signed access tokens are invalidated.** The signing-key plane moves to RSA / RS256 — re-issued tokens carry a `kid` header and are verified against the deployment's published JWKS (`<control_plane>/.well-known/jwks.json`). Every active session must re-authenticate; cached tokens from prior releases will fail validation.
- **Operators must publish a JWKS document.** Multi-tenant deployments and any caller that wants its tokens accepted by a peer deployment must serve `/.well-known/jwks.json` at the control-plane URL. Trusted-issuer entries must also point at a reachable, HTTPS-only JWKS endpoint; non-HTTPS or unlisted hosts are rejected by the JWKS client.
- **`jwt_secret` is removed from `Secrets` and `CloudTenantSecrets`.** It was dead weight after the RS256 cutover — no production path consumed it, but operators still had to supply a 32-character HMAC secret. The field, `JWT_SECRET_MIN_LENGTH`, `SecretsBootstrap::jwt_secret()`, the `JwtSecretRequired` error, and the `JWT_SECRET` env propagation through `infra/config` are all gone. Subprocesses now read `OAUTH_AT_REST_PEPPER`; operators must delete `jwt_secret` from `secrets.json` and ensure `oauth_at_rest_pepper` (>= 32 chars) is set.
- **`oauth_at_rest_pepper` is now a required deployment secret.** Refresh-token ids and authorisation codes are stored as the lowercase-hex HMAC-SHA-256 of the raw value under this pepper; a database read no longer yields live credentials. Migration `006_at_rest_pepper_hash.sql` invalidates pre-pepper rows on first migrate (active clients re-authenticate once).
- **`systemprompt_models::GrantType` is removed.** The canonical 4-variant enum (`AuthorizationCode`, `RefreshToken`, `ClientCredentials`, `TokenExchange`) lives in `systemprompt_oauth`. Downstream code that imported `systemprompt_models::GrantType` must switch to `systemprompt_oauth::GrantType`.

### Added

- **Actor attribution across the platform.** `Actor` and `ActorKind` live in `systemprompt-identifiers` and carry both the accountable principal (`user_id`) and the surface that ran on their behalf (`User`, `Anonymous`, `System`, `Job { job_name }`, `Mcp { server_name }`, `Agent { agent_id }`). Smart constructors — `Actor::user`, `Actor::anonymous`, `Actor::system`, `Actor::job`, `Actor::mcp`, `Actor::agent`, `Actor::from_tool_name` — and a single `Actor::audit_columns()` accessor make it impossible for the persisted `(user_id, actor_kind, actor_id)` triple to drift across insert sites. `Actor::from_tool_name` recovers the MCP surface from Claude Code's `mcp__<server>__<tool>` naming and attributes non-MCP tool calls to the agent surface when an agent id is present, falling back to the user — never to a sentinel.
- **Actor propagated through `ToolContext` and `JobContext`** in `systemprompt-provider-contracts`. AI tool invocations and scheduled jobs carry the acting `Actor` end-to-end; the CLI and API seed a bootstrap actor from the admin user; logging fields carry the actor on every span.
- **`UserId::admin()` and the `UserId::new` / `UserId::bootstrap` split.** Bootstrap admin owner is a typed identifier; `UserId::new` is the fallible runtime constructor; `UserId::bootstrap` is the compile-time form. `bootstrap_admin_owner()` is removed in favour of `UserId::admin()`.
- **RFC 8693 token exchange with `act_chain`.** `JwtClaims` and `RequestContext` carry an act-chain; security middleware persists it on audit rows; `profile.security.trusted_issuers` and `profile.security.signing_key_path` configure the validator. `/.well-known/jwks.json` is published on the API server; a JWKS HTTP client uses a bounded LRU with an HTTPS allowlist; `systemprompt admin keys generate` mints the RSA keypair. The grant is exposed at `/oauth/token` with `grant_type=urn:ietf:params:oauth:grant-type:token-exchange`; it validates the `subject_token` against the trusted-issuer registry (or the deployment's own signing key for self-issued tokens), intersects requested `scope` with the subject's scope, the client's scope grant, and the client owner's role set, and mints a delegated token whose `act` claim records the calling client. Pre-existing `act` chains on the subject token are preserved and chained underneath.
- **Multi-issuer JWT validation.** `profile.security.trusted_issuers` propagates onto the runtime `Config`; the token-exchange path consults the registry to resolve issuer → JWKS URI → signing key for non-self-issued tokens.
- **`invalid_target` rejection** per RFC 8707 when `resource` or `audience` falls outside `allowed_resource_audiences`.
- **`generate_jwt_with_act`** in `systemprompt-oauth` mints a token carrying an explicit `ActClaim` outermost link, chaining any prior `act` underneath.
- **HMAC-SHA-256 at-rest hashing for OAuth identifiers.** `systemprompt-security` exposes `hmac_sha256` and `hmac_sha256_hex` via a new `at_rest` module. `oauth_refresh_tokens.token_id`, `oauth_refresh_tokens.family_id` (when seeded from `token_id`), `oauth_auth_codes.code`, and `oauth_auth_codes.refresh_token_id` now hold the digest, not the raw identifier. Schema is unchanged; storage value is.
- **Federated identities.** New `federated_identities` schema in `systemprompt-users` and `UserProvider::find_or_create_federated` lookup-or-provision API for federated subjects arriving via token exchange.
- **OAuth client ownership.** `oauth_clients` rows carry `owner_user_id`; bridge and admin client-create flows thread the owner through; `client_credentials` tokens are minted as the client's owner so audit attribution is consistent end-to-end.
- **Session hardening.** JWT middleware enforces session existence on every authenticated request; `/oauth/revoke` revokes the session via the analytics provider; auth-code replay detection revokes the entire refresh-token family; `POST /api/v1/core/users/me/sessions/revoke_all` lets a user invalidate every session from any device.
- **`infra db migrate-mark-applied`.** Marks a migration as applied without running its SQL, for partial-state recovery scenarios where a migration ran out-of-band and the bookkeeping row needs to catch up.
- **Shared `systemprompt-test-fixtures` crate.** All placeholder `UserId` construction in tests routes through it; the previous per-crate sentinels are gone.
- **Service-JWT sync auth.** New `sys_sync` OAuth client and `provision_sync_oauth_client` service in `systemprompt-oauth`; `app/sync` API client requests tokens via `client_credentials` and rotates them on expiry. Headers and identifier types are typed end to end (`ClientId::sync()`, `ClientId::bridge()`).
- **`RouterExt::with_auth(_, AuthzPolicy::*)` middleware gate** in `entry/api`. Every authenticated route declares its policy at registration, making authz tier a compile-time requirement; the runtime no longer silently accepts a route that forgot to install a guard.
- **Postgres event outbox.** `infra/events` ships an outbox repository plus a `LISTEN`/`NOTIFY` bridge on the `systemprompt_events` channel (`OUTBOX_CHANNEL` constant). Domain events written by one replica are observed by every other replica subscribed to the bus.
- **Cross-replica scheduler job claim.** `app/scheduler` claims cron jobs through Postgres advisory locks keyed by job + tick; concurrent replicas race for the lock and at most one executes a given tick. A tick-deterministic key removes the previous time-drift flake.
- **Read-replica routing.** `infra/database` honours an optional read-replica URL and routes read-only queries to it; writes continue to land on the primary. New `infra db migrate-repair` reconciles checksum drift in place.
- **Typed identifiers across `domain/agent`.** Context, task, message, and notification repositories take `&ContextId` / `&AgentId` / `&TaskId` / `&MessageId` parameters end to end; raw `&str` IDs are gone from the agent surface.
- **`JsonSchema` derives across the profile config tree** in `systemprompt-models` — `profile/{security,governance,runtime,gateway,server,cloud,site,paths,...}`. Profiles can now be introspected and validated against a generated schema.
- **Prometheus metrics endpoint** on the API server, plus a `services/middleware/served_by.rs` middleware tagging responses with the serving replica identity for load-balancer fairness measurement.
- **Replica identity and stream-concurrency config** in `app/runtime`. A global semaphore bounds in-flight A2A SSE streams so a single replica can't exhaust file descriptors under fan-out.
- **Load-test `lb_fairness` scenario and text reporter** in the load-test harness, alongside the air-gap profile shipped earlier in this cycle: distributed-runner, soak, and spike profiles round out the multi-replica validation matrix.
- **Air-gapped deployment test tooling.** A new `mock-inference` test crate stands in for an internal OpenAI/Anthropic-compatible inference endpoint — Anthropic Messages and OpenAI Chat wire formats, streaming and non-streaming, with configurable latency, failure injection, and a request counter exposed at `GET /stats`. The load-test harness gains an `airgap` profile with strict latency thresholds, `gateway-inference` and `governance-only` scenarios, a JSON reporter (`--output json` / `--out-file`), and a `--admin-email` flag (also read from `SYSTEMPROMPT_ADMIN_EMAIL`). New `just` recipes `mock-inference` and `loadtest-airgap`.

### Changed

- **AI gateway repositories use compile-time-verified query macros** (`query!`, `query_as!`, `query_scalar!`) instead of dynamic `query(_)` + `bind(_)`. The runtime-SQL carve-out in `crates/infra/database/src/admin/**` remains the only legal home for dynamic SQL in domain repositories.
- **`entry/api` strips every per-item `///` rustdoc** in line with the standing rustdoc rule — handlers, middleware, and binary modules describe their purpose through file-level `//!` blocks where the value is real, not by paraphrasing each function signature.
- **`apply_notification_status` and `message_exists` retyped end-to-end** through the handler chain to consume the agent crate's typed identifiers; no raw `String` IDs reach the repository.
- **`Authorization` and image OpenAI provider call sites** cleaned up of `clippy::useless_borrows_in_formatting` and redundant `&` in `format!` arguments.
- **Per-item `///` paraphrase docs stripped across library crates.** `synthesize_route_id`, `SchedulerConfig::with_system_admin`, `DenyAllHook::null`, `AllowAllHook::null`, `AuthzRepository::set_justification`, `CircuitBreaker::new`, `Bulkhead::new`, `ResilienceGuard::new`, `AiError::from_error_response`, `AiError::classify`, `AiGatewayPoliciesRepository::delete_by_name`, `platform_attribution`, and `http_client::build_client` no longer carry rustdoc that paraphrases the function name; non-obvious WHYs were preserved as `//` body comments where they existed.
- **`list` alias added to canonical "show contents" subcommands.** `admin config {paths,runtime,security,rate-limits} show`, `web sitemap show`, `infra db migrations status`, `analytics sessions stats`, `analytics content stats`, `analytics traffic sources`, and `analytics costs summary` now accept `list` as an alias. `admin session show` accepts `current`; `cloud auth whoami` accepts `status`. Non-breaking; canonical verbs remain.
- **`plugins mcp call --help` shows a worked `--args` example** so the JSON-string contract is discoverable without trial and error.
- **`admin agents message --help` documents the `-m` single-quoting rule** so flag-like tokens (`--foo`) inside the message text don't end the `-m` value.

### Removed

- **`release-sign` and `sbom` GitHub workflows.** `systemprompt-bridge` binaries — including CycloneDX SBOMs and cosign signatures — are now produced through a manual release process rather than on every `v*` tag push.
- **Dead `ToolProvider` trait and `AiServiceToolProvider`** in `systemprompt-agent`, and unused dead-code fields surfaced by the standards pass.
- **Unused `_db_pool` / `_ctx` plumbing removed** from `AiService` (held field), `McpClient::call_tool`, `monitor_health_continuously`, `McpToolLoader`, `MonitoringHandler::new`, `generate_feed_with_providers`, and `load_service_configs`. `MonitoringHandler` now derives `Default` instead of taking a constructor argument; the orchestrator registers it as `Arc::new(MonitoringHandler)`.

### Fixed

- **RS256 token verification: `DecodingKey` now built from PKCS#1 RSAPublicKey DER, not SPKI.** `crates/infra/security/src/keys/authority.rs::build()` was calling `signing_key.public_key().to_public_key_der()` (which produces SubjectPublicKeyInfo DER) and feeding the bytes into `jsonwebtoken::DecodingKey::from_rsa_der`, which expects raw PKCS#1 `RSAPublicKey` DER. Every RS256 signature verified as `InvalidSignature` even though CLI and server held the identical private key under the same `kid`. Switched to `EncodeRsaPublicKey::to_pkcs1_der()` so encoding and decoding keys are derived from matching DER forms; the unused `SpkiEncode` variant and `pkcs8::EncodePublicKey` import are gone.
- **Agent subprocess installs `SystemAdmin` before resolving MCP tools.** `crates/entry/cli/src/commands/admin/agents/run.rs::execute()` now calls `AppContext::new().await` before constructing `McpToolProvider`. Without it, the agent subprocess skipped the typed-SystemAdmin install and every MCP tool resolution failed with `"system admin not resolved: AppContext bootstrap must run before any system-attributed work"`. The CLI agent server path now mirrors the API server's bootstrap.
- **MCP RBAC propagates `act_chain` to the authenticated `RequestContext`.** Previously the chain was extracted for the authorization decision and dropped before building the per-request context, leaving MCP audit rows without delegation attribution while the REST middleware persisted the full chain. MCP and REST now behave identically.
- **Authz `bootstrap.rs` tests are no longer flaky.** A shared global hook slot is now serialised via a process-wide `tokio::sync::Mutex`; concurrent test execution no longer sees half-installed hooks.
- **CI `test` job no longer inherits the workspace `-D warnings` clippy level**, so a warning surfaced by a sibling crate doesn't fail an unrelated test job. The dedicated `lint` job continues to enforce `-D warnings`.
- **CHANGELOG-drift error path is actionable**: the database migration runner points operators at `infra db migrate-repair --apply` instead of dead-ending on the previous "use `--allow-checksum-drift`" hint, which suppresses the symptom without reconciling the drift.
- **Stricter `UserId` parsing on the OAuth and WebAuthn paths.** Invalid identifier inputs return a typed parse error rather than being coerced.
- **Tighter authentication scope on OAuth endpoints.** `/oauth/register` and `/oauth/introspect` now require client authentication; `/oauth/introspect` responses are limited to the authenticated client's own subjects.
- **Scheduler lock probe takes an explicit owner.** The distributed-lock test path no longer relies on a placeholder `UserId`.
- **`entry/api` service-proxy auth wired through `AppContext::mcp_registry()`.** `lookup_oauth_requirement` previously tried to construct `RegistryManager` as if it were a unit struct and to call its `validate()` as an associated fn, both of which mismatched the current shape of the type. It now obtains the registry through the bootstrapped `AppContext` and invokes the method on the instance, matching the API-server bootstrap.
- **Bootstrap advisory lock now released on the session that acquired it.** Extension-schema installation in `systemprompt-database` previously called `pg_advisory_lock` and `pg_advisory_unlock` through `DatabaseProvider::execute_raw`, which checks out a fresh pooled connection per call. The unlock ran on a different Postgres session than the lock, logged `WARNING: you don't own a lock of type ExclusiveLock`, and returned `false`; the lock survived until the original pooled connection was recycled. A new `BootstrapLockGuard` pins one `PoolConnection<Postgres>` for the install's lifetime so acquire and release run on the same session.
- **`infra/events` enables the `sqlx` feature on `systemprompt-identifiers`.** The outbox repository binds `user_id` as a typed `UserId` in its `query_as!` macro; without the feature flag `UserId` lacked `sqlx::Decode<Postgres>` and the workspace failed to compile under `cargo package` / offline check.
- **`plugins mcp list-packages` and `plugins mcp logs` bootstrap as full-init commands.** Both were classified `PROFILE_ONLY` in `entry/cli`'s `PluginsCommands::descriptor`, which skips `init_paths()`; the handlers then called `AppContext::new()` and tripped `Config not initialized. Call Config::init() first.`. They join `list` / `status` / `validate` / `tools` / `call` on the `FULL` arm.
- **`plugins show <id>` resolves the same set `plugins list` enumerates.** Previously only compiled-registry extensions keyed by `ext.id()` were lookup-able; ids printed by `list` for manifest-loaded extensions or whose runtime `id()` differed from their `name()` returned `Extension not found`. `show` now matches case-insensitively against both `id()` and `name()` in the compiled registry and falls back to the manifest set discovered by `ExtensionLoader::discover`.
- **`infra logs trace list` resolves trace status from the most recent `agent_tasks` row.** The CTE that gathers trace ids now also reads from `agent_tasks` (filtering empty-string `trace_id`s); the per-trace `agent` and `status` subqueries order by `updated_at DESC` so the picked row reflects the latest known state instead of an arbitrary one. Traces that only produced an `agent_tasks` row are now visible; `status` no longer collapses to `"unknown"` when the picked row happens to be a stale `TASK_STATE_SUBMITTED`.
- **`ai_requests.trace_id` propagation failures surface in the operator log.** `systemprompt-ai`'s record builder now emits a `WARN` carrying `request_id` when the supplied `RequestContext` carries an empty `trace_id`, instead of silently omitting the column and breaking the trace ↔ `ai_requests` join in `infra logs trace list`.
- **Cloud credential bootstrap is gated on the resolved command path.** `init_credentials_gracefully` was previously called for every CLI invocation, so an expired or missing cloud token printed a `WARN` ahead of `--help`, `--version`, `--json` output, and every non-cloud command. The bootstrap now runs only for `cloud *`, `admin session *`, and cloud profiles using `external_db_access`; other paths stay silent.
- **CLI profile banner only renders under `--verbose` or non-local profiles.** Default `infra services status` / `core skills list` invocations no longer prefix output with `[profile: local (local) | tenant: ...]`.
- **`rmcp` transport logs default to `WARN`.** `rmcp::service` and `rmcp::transport::streamable_http_client` `INFO` lines no longer leak into `plugins mcp call` output. `RUST_LOG` and `--verbose` continue to surface them.

### Changed

- **`RequestStorage::store` in `systemprompt-ai` is now async and fallible.** The primary `ai_requests` insert previously ran inside a detached `tokio::spawn`, so a broken trigger or schema drift was silently swallowed and never reached the caller. `store` is now an `async fn` returning `Result<(), AiError>`; every `AiService` path awaits it and propagates the error. Secondary writes (per-message rows, per-tool-call rows, session usage, analytics events) remain best-effort and log on failure. The internal module `request_storage::async_operations` is renamed to `request_storage::writes` to reflect that the calls are no longer detached.

### Docs

- **Security documentation refreshed to the RS256 / RFC 8693 reality.** `documentation/compliance-control-matrix.md` (ISO 27001:2022 A.8.24), `documentation/threat-model.md` (STRIDE spoofing/tampering rows), and `documentation/deployment-reference-architecture.md` (§6 key rotation, §8 air-gap) previously described HS256 + shared-secret JWT verification with RS256 as roadmap; they now describe the in-process `TokenAuthority`, `/.well-known/jwks.json`, `profile.security.trusted_issuers`, the Ed25519 manifest signing key, and the OAuth at-rest pepper.
- **`.cargo/audit.toml` note on RUSTSEC-2023-0071** (rsa Marvin Attack) rewritten: the previous justification ("every JWT site pins HS256, so the rsa code path is unreachable") no longer holds. The note documents the accepted risk and the mitigation options (`aws_lc_rs` backend, ES256/EdDSA, or upstream rsa fix).
- **`crates/infra/security/README.md`** stripped of HS256 / `jwt_secret` examples (`SessionGenerator::new(jwt_secret, …)`, `AuthValidationService::new(secret, …)`, `AdminTokenParams { jwt_secret, … }`) — none of those signatures exist post-cutover. Replaced with the current `TokenAuthority`-backed APIs.

## [0.10.3] - 2026-05-18

### Added

- **`infra db migrate-repair`** reconciles migration checksum drift in place. When an already-applied migration file is edited, its stored checksum stops matching the file and `infra db migrate` halts. The command drops the drifted bookkeeping rows and re-applies those migrations — every migration is idempotent, so re-running re-records the current checksum without touching data. It lists drift as a dry-run by default; `--apply` performs the repair and `--extension <id>` limits it to one extension. `MigrationService::repair_drift` exposes the same operation to library callers.

### Changed

- The migration checksum-drift error now points at `infra db migrate-repair --apply` to reconcile the tracking table, rather than only `--allow-checksum-drift` — which suppresses the error without resolving the drift, so it recurs on every boot.
- `McpToolHandler` is now a native `async fn` trait. The trait has associated types and is never `dyn`-compatible, so `#[async_trait]` was unnecessary. Implementors must drop the `#[async_trait]` attribute from their `impl` block and write `handle` as a plain `async fn`.

### Removed

- The dead `ToolProvider` trait and `AiServiceToolProvider` in `systemprompt-agent`, which had no callers.

## [0.10.2] - 2026-05-15

Database lifecycle hardening: transactional migrations, reversible migrations, an AST-based schema linter, a cross-extension table-ownership contract, post-migration seeds, dependency-ordered extension loading, squash tooling, connection retry, and introspectable migration status.

### Added

- **Transactional migrations.** Each migration runs inside a single `BEGIN`/`COMMIT` envelope; on failure the runner issues `ROLLBACK` and does not record the migration, so a partially-applied migration is no longer possible. `Migration::new_no_transaction` opts a migration out of the envelope for statements that cannot run inside a transaction block (for example `CREATE INDEX CONCURRENTLY`).
- **Reversible migrations.** `Migration` gains an optional `down` field; construct with `Migration::with_down(version, name, up, down)`. `MigrationService::run_down_migrations` reverts the most recently applied migrations, and `infra db migrate down <extension> <count>` exposes this on the CLI. Reverting a migration with no `down` SQL fails with `LoaderError::MigrationNotReversible`.
- **Table-ownership contract.** Each extension's owned tables are derived from the `CREATE TABLE` statements in its `schemas()`; `Extension::cross_extension_tables()` declares tables owned elsewhere that its migrations may legally `ALTER`. Schema installation rejects two extensions that create the same table (`LoaderError::DuplicateTableOwner`), a `cross_extension_tables()` entry no other loaded extension creates (`LoaderError::CrossExtensionTableNotOwned`), and an undeclared cross-extension `ALTER` in a migration (`LoaderError::CrossExtensionAlterUndeclared`).
- **Schema-qualified declarative schemas.** `SchemaDefinition::with_schema` places a definition's table in a non-`public` Postgres schema; required-column validation queries the declared schema instead of assuming `public`.
- **Post-migration seeds.** `Extension::seeds()` returns idempotent `Seed` values applied after migrations on every boot and intentionally not tracked in `extension_migrations`. Seed SQL is restricted to `INSERT … ON CONFLICT` / `UPDATE` / `MERGE`; `CREATE`/`ALTER`/`DROP` are rejected.
- **Dependency-ordered extension loading.** The extension registry topologically sorts by `Extension::dependencies()` before falling back to `priority()`. A dependency cycle fails with a typed `LoaderError::DependencyCycle` naming the offending chain; a missing dependency warns and is skipped.
- **`infra db migrate plan`** lists pending migrations without applying them, and **`infra db migrate status`** reports applied and pending migrations plus checksum drift. Both render a text table or, with `--json`, structured output.
- **`infra db migrate squash --extension <id> --through <N>`** concatenates an extension's first `N` migrations into a `000_baseline_v{N}.sql` file and, with `--apply`, retires their bookkeeping rows behind a synthetic version-0 baseline. It is a dry-run by default and refuses to run unless migrations `1..=N` are all already applied.
- **First-connect retry.** The initial database connection retries transient failures (connection refused, the SSL-handshake race, and "starting up") with exponential backoff at 100/200/400/800ms, capped at five attempts. Non-retryable errors such as authentication failures fail immediately; every attempt is logged at `WARN`.
- **Filename-driven extension migrations.** Migration SQL is discovered from `<crate>/schema/migrations/NNN_<name>.sql` files at build time. An extension crate adds a one-line `build.rs` calling `systemprompt_extension::build::emit_migrations()` and returns the new `extension_migrations!()` macro from `Extension::migrations()`; each migration's version and name are derived from its filename. A `-- @no-transaction` first line and a paired `NNN_<name>.down.sql` file select `Migration::new_no_transaction` and `Migration::with_down`.
- **`just lint-extensions`** rejects SQL string literals and hand-written `Migration::new(...)` calls in `extension.rs` files, keeping schema DDL and migration SQL in `schema/` files. Wired into `just check`.

### Changed

- **`Extension::migration_weight()` removed.** Extension and schema-install order is derived solely from the dependency graph — `Extension::dependencies()`, tie-broken by `priority()`. An extension that relied on `migration_weight` must declare the dependency it was implicitly ordering behind. `SchemaExtensionTyped::migration_weight()` and `LoaderError::InvalidDependencyOrdering` are removed with it.
- **The schema-install phase classifier is exhaustive.** Every `pg_query` DDL node variant is matched explicitly when splitting a schema into structural and dependent statements; an unrecognised node fails installation rather than being silently mis-phased.
- **The declarative schema linter is now parser-based.** `schema_linter` parses each schema with `pg_query` and classifies statements by AST node variant rather than a hand-rolled keyword scanner. It additionally resolves column references in `CREATE INDEX` and view definitions against sibling `CREATE TABLE` statements and rejects unknown columns at lint time (`LintError::UnknownColumn`). This adds a C-toolchain build dependency (`pg_query`/`libpg_query`). Schemas that previously passed the keyword scanner but reference a column not declared in the same extension's schema files will now fail `just lint-schema`.
- **The schema linter permits `DROP` of a stateless derived object** — `VIEW`, `MATERIALIZED VIEW`, `INDEX`, or `TRIGGER` — when guarded by `IF EXISTS`. Such a drop loses no data and is rebuilt by the sibling `CREATE` statement. `DROP TABLE` and `DROP COLUMN` remain rejected in declarative schemas.
- **Breaking — `ExtensionRegistry::discover()` now returns `Result<Self, LoaderError>`** instead of `Self`, and `ExtensionRegistry::register` propagates the same error. A dependency cycle among extensions surfaces as `LoaderError::DependencyCycle` rather than a panic. Callers migrate by appending `?` (the error implements `std::error::Error`).
- **Bundled extensions no longer hold migration SQL in Rust source.** The `oauth`, `analytics`, and `users` extensions moved their inline migration string constants into `schema/migrations/*.sql` files; every bundled extension now returns `extension_migrations!()` from `Extension::migrations()`. Migration checksums are byte-for-byte unchanged, so databases that already applied these migrations see no drift.

### Fixed

- **Schema installation no longer fails on legacy databases when a schema references a migration-added column.** Installation runs in three global phases — structural DDL (`CREATE SCHEMA`/`TABLE`/`TYPE`/`EXTENSION`), then all pending migrations, then dependent DDL (`CREATE INDEX`/`VIEW`/`FUNCTION`/`TRIGGER`). Previously a schema's `CREATE INDEX` or `CREATE VIEW` could run before the migration that added the column it references. Because every table now exists before the migration phase, a migration may also `ALTER` a table owned by another extension.
- **User-session analytics views install correctly on databases carrying the previous view shape.** PostgreSQL's `CREATE OR REPLACE VIEW` cannot rename or reorder output columns; the views are now dropped with `DROP VIEW IF EXISTS … CASCADE` before recreation, so a column rename no longer fails at install time.
- **The `database` extension installs ahead of every other extension.** With `migration_weight` removed, install order is the dependency graph tie-broken by `priority()`; no extension declares `database` as a dependency, so the shared `update_timestamp_trigger()` helper and the `extension_migrations` table could be ordered after an extension that needs them. `DatabaseExtension` now declares the lowest priority so it always installs first.

## [0.10.1] - 2026-05-14

### Fixed

- **Pending migrations now run before an extension's declarative schema is installed**, so a legacy database reaches the target table shape before the schema's `CREATE … IF NOT EXISTS` statements run.
- **The CLI degrades gracefully on expired or invalid cloud credentials** instead of failing startup outright.

## [0.10.0] - 2026-05-14

Friction-reduction follow-ups from the 0.9.2 fresh-clone retro plus a structural rule for schema files. Bumped to a minor because schema files now have a hard linter at boot and `SqlExecutor::parse_sql_statements` changes its public return type.

### Breaking

- **Schema files must be purely declarative.** `<crate>/schema/<name>.sql` may contain only `CREATE TABLE IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`, `CREATE [OR REPLACE] FUNCTION/VIEW/TRIGGER`, `CREATE TYPE`, `CREATE EXTENSION IF NOT EXISTS`, and `COMMENT ON`. `ALTER`, `DROP`, top-level `DO $$ … $$`, `UPDATE`/`INSERT`/`DELETE`, `TRUNCATE`, `GRANT`, and `REVOKE` are rejected at install time by `schema_linter::lint_declarative_schema` in `crates/infra/database`. Imperative state transitions move to `<crate>/schema/migrations/NNN_<name>.sql` declared via `Extension::migrations()`. The runner applies pending migrations BEFORE executing each extension's schema, so legacy databases reach the target shape before the schema's `CREATE … IF NOT EXISTS` runs. Pre-merge gate: `just lint-schema` (wired into `just check`). See `instructions/information/migrations.md`.

### Changed

- **Breaking — `SqlExecutor::parse_sql_statements` now returns `DatabaseResult<Vec<String>>` instead of `Vec<String>`.** The hand-rolled line scanner in `crates/infra/database/src/services/executor.rs` is replaced with `sqlparser::Parser::parse_sql(&PostgreSqlDialect, …)`. Named dollar-quoted bodies (`$body$ … $body$`) and apostrophe-quoted function bodies are now handled correctly; the previous heuristic only matched `$$`. Unparseable SQL surfaces as `RepositoryError::Internal` rather than silently producing a truncated statement list. The two helper functions `should_skip_line` / `is_statement_complete` are gone. All three call sites (`services/executor.rs:execute_statements_parsed`, `lifecycle/installation/extension.rs:install_extension_schema`, `services/postgres/mod.rs:execute_batch`) propagate the new `Result`; the installation site maps the parse error into `LoaderError::SchemaInstallationFailed`. Schemas under `crates/**/schema/*.sql` are dialect-clean; this is a strict-mode upgrade, not a behavioural regression.
- **`init_credentials_gracefully` no longer pattern-matches `CloudError::CredentialsFileNotFound` directly** (`crates/entry/cli/src/bootstrap.rs`). It calls a new `CloudError::is_missing_credentials_file()` predicate instead, so future renames or refactors of the variant don't silently regress the fresh-clone fallback path (the exact regression class that broke 0.9.1). A matching `CredentialsBootstrapError::is_file_not_found()` is added for symmetry.
- **`instructions/information/crates-publishing.md` leads with `just release patch`** instead of the raw `./scripts/release.sh patch` invocation. The script itself stays gitignored.

### Added

- **`just release [patch|minor|major]` recipe** in the root `justfile`. Validates the bump kind, checks `scripts/release.sh` is present and executable, then delegates. The release script remains local-only; the recipe is the discoverable entry point referenced from the publishing doc.
- **`CloudError::is_missing_credentials_file()` / `CredentialsBootstrapError::is_file_not_found()`** — inherent `const` helpers, no new traits, no dyn overhead. Two regression tests live in `crates/tests/unit/infra/cloud/src/error.rs`.

### Fixed

- **`parse_sql_statements` mishandled named dollar quotes and bare-apostrophe function bodies.** The previous scanner only looked for `$$`, so a `CREATE FUNCTION … AS $body$ … $body$ LANGUAGE plpgsql;` block (or an apostrophe-quoted body) followed by another statement was treated as a single concatenated statement and rejected by sqlx. Three regression tests added to `crates/tests/unit/infra/database/src/services/executor.rs`: named `$tag$` bodies, apostrophe bodies, and malformed SQL surfacing `Err` instead of producing a garbage statement.

## [0.9.2] - 2026-05-12

### Fixed

- **Fresh-clone bootstrap aborted when `.systemprompt/credentials.json` was absent.** `crates/entry/cli/src/bootstrap.rs::init_credentials_gracefully` previously downcast the underlying anyhow error to `CredentialsBootstrapError::FileNotFound`, but the 0.9.1 refactor of `CredentialsBootstrap::init` to return `CloudResult` meant the error reaching the call site was the converted `CloudError::CredentialsFileNotFound` variant — the downcast missed and the CLI failed strictly instead of falling back to `init_empty()`. The graceful wrapper now calls `CredentialsBootstrap::init()` directly and matches on `CloudError::CredentialsFileNotFound` by pattern, removing the brittle dual `downcast_ref` and the unused `init_credentials()` helper.
- **Schema install on a clean database failed on `CREATE TRIGGER` statements.** `SqlExecutor::parse_sql_statements` (`crates/infra/database/src/services/executor.rs`) treated `CREATE TRIGGER` as opening a plpgsql function body, so it kept appending lines until it saw `END;` / `LANGUAGE plpgsql;` — neither sentinel ever appears in a Postgres trigger (triggers always reference a separate function with `EXECUTE FUNCTION foo();`), so the trigger and every subsequent statement got concatenated into one prepared statement that sqlx rejected. The body-detection branch now fires only on `CREATE [OR REPLACE] FUNCTION`; the internal flag was renamed `in_trigger` → `in_function_body` so the misuse is harder to reintroduce. Regression-covered by `crates/tests/unit/infra/database/src/services/executor.rs`.

### Changed

- **Schema-install pipeline overhaul (Phases 1–4).** Single coherent change to how every `Extension` reaches a live Postgres on boot. Production impact: the prod content_ingestion incident where `markdown_content.locale` was missing despite core 0.9 shipping the safety-net ALTER cannot recur — the installer no longer skips idempotent ALTERs on already-existing tables.
  - **Phase 1 — always-run schemas.** `install_extension_schema` (`crates/infra/database/src/lifecycle/installation/extension.rs`) no longer short-circuits when a schema's primary table already exists. Every `SchemaDefinition.sql` runs on every boot. Schemas are expected to be idempotent (`CREATE TABLE IF NOT EXISTS`, `ADD COLUMN IF NOT EXISTS`, `CREATE INDEX IF NOT EXISTS`) — the previous skip silently dropped every post-install ALTER on legacy tenants.
  - **Phase 2 — transactional install + surfaced errors.** All parsed statements for one extension execute inside a single transaction via `db.begin_transaction()`. On per-statement failure the transaction rolls back and the error carries `Statement N/M failed: …\nSQL: <text>` with the offending statement text. The previous batch-then-per-statement fallback (which could commit partial DDL) is gone.
  - **Phase 2 — checksum drift is a hard error.** `MigrationService::run_pending_migrations` (`crates/infra/database/src/lifecycle/migrations.rs`) returns `LoaderError::MigrationFailed` when a stored migration's checksum no longer matches the SQL on disk. New `MigrationConfig { allow_checksum_drift }` and `MigrationService::with_config` let admins explicitly opt out via `systemprompt infra db migrate --allow-checksum-drift`.
  - **Phase 2 — dependency-weight validation.** `ExtensionRegistry::validate_dependencies` (`crates/shared/extension/src/registry/validation.rs`) now requires every declared dependency to have a strictly lower `migration_weight()` than its dependent, surfacing FK-ordering bugs at registry build instead of install. New variant `LoaderError::InvalidDependencyOrdering { extension, extension_weight, dependency, dependency_weight }`.
  - **Phase 3 — schema install is part of `AppContext`.** New builder hooks `AppContextBuilder::with_migrations(bool)` and `with_migration_config(MigrationConfig)` (`crates/app/runtime/src/builder.rs`) run `install_extension_schemas_full` during `build()`. `systemprompt serve` (`crates/entry/cli/src/commands/infrastructure/services/serve.rs`) sets `with_migrations(true)`; the standalone `run_migrations` helper is gone.
  - **Phase 3 — advisory lock around install.** `install_extension_schemas_full` takes Postgres advisory lock `0x73706F6D70740 1` for the duration of the install pass and releases it on completion (and on error). Rolling deploys can no longer race on idempotent DDL.
  - **Phase 4 — `SchemaSource` enum collapsed to `String`.** `SchemaDefinition.sql: String` (was `enum { Inline(String), File(PathBuf) }`); single constructor `SchemaDefinition::new(table, sql)`. Same applies to `SchemaDefinitionTyped` in the typed path. Every production extension migrated.
  - **Phase 4 — dead YAML-loader subsystem removed.** Deleted `crates/infra/database/src/lifecycle/installation/{module.rs,util.rs}`, `crates/app/runtime/src/installation.rs`, `crates/shared/models/src/modules/types.rs`, `crates/shared/models/src/errors/module.rs`. Public types removed: `Module`, `Modules`, `ModuleDefinition`, `ModuleSchema`, `ModuleSeed`, `ApiConfig`, `ModulePermission`, `SeedSource`, `SchemaSource`, `ModuleRuntime`, `ModuleInstaller`, `install_module`, `install_module_with_db`, `install_module_schemas_from_source`, `install_module_seeds_from_path`, `install_schema`, `install_seed`, `ModuleError`. The dead `AppContext::get_provided_audiences` / `get_valid_audiences` / `get_server_audiences` accessors are also gone (zero non-test callers across core, web, and template). `ModuleType` (Regular/Proxy) is preserved by moving it to `crates/app/runtime/src/registry.rs` where its sole consumers live.

### Added

- **`systemprompt infra db doctor`** (`crates/entry/cli/src/commands/infrastructure/db/doctor.rs`). Read-only drift report: lists tables that exist in `information_schema` but are not declared by any registered extension, declared tables absent from the live database, and declared `required_columns` missing from live tables. Text and JSON output via existing `CommandResult` plumbing.
- **`instructions/information/migrations.md`** — workflow doc for shipping additive vs versioned schema changes, advisory-lock behaviour, dependency-ordering rules, and a triage table for common failure modes.

### Fixed

- **Test workspace pool exhaustion under parallel execution.** `crates/tests/unit/domain/analytics/src/repository/costs.rs` opened a 50-connection sqlx pool per test; cargo's default parallel scheduler put ~8 × 50 = 400 connection requests against `max_connections=100`, timing out the late tests with "pool timed out while waiting for an open connection". Tests in that module now serialise against an in-process `tokio::sync::Mutex` gate carried in `Fixture`. Total wall-clock for the 8 tests is 1.25 s, so the serialisation cost is negligible.

## [0.9.1] - 2026-05-12

### Added

- **Handlebars `json` helper** (`crates/domain/templates/src/registry/mod.rs`). Registered on every `TemplateRegistry::new()`. `{{{json field}}}` emits values via `serde_json::to_string`, correctly escaping backslashes, quotes, newlines, and control characters that Handlebars' default HTML escaping leaves intact — required for safe `<script type="application/ld+json">` and other inline-JSON contexts. Non-string values (numbers, bools, objects) round-trip unquoted.

### Changed

- **Cloud credentials bootstrap error is actionable.** `CredentialsBootstrap` now surfaces an operator-targeted message ("tenant pod credentials rejected by api.systemprompt.io … re-run `systemprompt cloud deploy` or set `SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS=1` to bypass") instead of the bare underlying error string. The redundant inner `map_err` in `validate_with_api` is removed — the outer call site owns the user-facing wording.

### Fixed

- **Test workspace caught up with 0.9.0 i18n.** Updated fixtures in `crates/tests/unit/domain/content/**` (added `locale: LocaleCode::new("en")` to `Content` initializers, `locale: None` to `ContentMetadata`) and `crates/tests/unit/app/generator/src/sitemap{,/xml,_tests}.rs` (added `alternates: vec![]` to all `SitemapUrl` literals; relaxed an XML assertion that hard-matched the old `<urlset>` opening tag, which now declares the `xhtml` namespace for hreflang `<xhtml:link>` alternates).

## [0.9.0] - 2026-05-08

### Changed

- **Marketplace consolidation: skills, agents, and hooks become file-driven first-class entities.** Three new categories (in addition to plugins and MCP servers) are now sourced directly from `<services_root>/{skills,agents,hooks}/<id>/config.yaml` and validated at startup. The DB ingestion hop has been removed and the corresponding tables dropped. RBAC grants for skills and hooks live in `access_control_rules` (the `entity_type` CHECK already accepts `'skill'` and `'hook'`).
  - **Skills →  disk.** `SkillService` now reads `<services_root>/skills/<id>/{config.yaml, SKILL.md}` directly. The A2A processing layer wires `SkillService::new()` with no DB pool; tracking is injected via `with_execution_step_repo`. Deleted: `SkillRepository`, `SkillIngestionService`, `agent::models::Skill`, `agent_skills.sql`, all `app/sync/{local,diff,export}/skills*` modules, and `cli/commands/cloud/sync/skills.rs`. CLI `core/skills/{create,delete,edit,sync,status,…}` are gone; only `list` and `show` remain. Migration `006_drop_agent_skills.sql` drops the table; `task_execution_steps.step_content` keeps `skill_id` as opaque text in JSONB with no FK, so the drop is safe.
  - **Marketplace agents → disk.** `AgentRegistry` (already YAML-driven) is now the sole source of truth for marketplace/persona agents. Deleted: `AgentRepository`, `AgentEntityService`, `AgentIngestionService`, `agent::models::Agent`, `database_rows::AgentRow`, `agents.sql`, all `app/sync/{local,diff,export}/agents*` modules, and `cli/commands/cloud/sync/agents.rs`. CLI core agents subcommand is removed. Migration `007_drop_agents.sql` drops the table; runtime tables (`services`, `agent_tasks`, `task_messages`, `context_agents`) keep `agent_name` as opaque text without FK, so the drop is safe. **A2A runtime agents are unchanged** — they are routed via `AgentRegistry` and never queried the dropped catalog table.
  - **Hooks → first-class.** `bridge_manifest::load_hooks` and the synthetic-plugin writer already read disk hooks; the CLI `core/hooks/{list,validate}` commands are now wired the same way (via `DiskHookConfig`) instead of walking `PluginConfig.hooks`. The `hooks: HookEventsConfig` sub-field has been removed from `PluginConfig` and the offline `plugin generate` no longer emits `hooks/hooks.json` (the bridge writes it from disk + manifest). Existing plugin `config.yaml` files with a `hooks:` section continue to deserialize cleanly — the unknown field is ignored.
- **`AgentRegistry` snapshot is now lock-free.** Replaces `Arc<RwLock<ServicesConfig>>` with `Arc<ServicesConfig>` and removes the unused `reload()` machinery. The lookup methods stay `async` for `AgentRegistryProvider` trait compatibility but their bodies are pure synchronous lookups.

### Added

- **`ContextId::derived_from_gateway_conversation`.** Stable UUID v5 derivation lets the gateway boundary mint a UUID-shaped `ContextId` per conversation without trusting upstream client `x-context-id` headers (which carry client-specific non-UUID values).
- **Multilingual (i18n) support for DB-backed content.** Framework-level primitives for serving content in multiple locales. `LocaleCode` validated newtype (BCP-47-lite) in `systemprompt_identifiers`. `markdown_content` gains a `locale` column with `UNIQUE (slug, locale)` and a dedicated index (migration in `markdown_content.sql`); `Content`, `CreateContentParams`, and the `ContentMetadata` frontmatter struct all carry `locale`. All repository read paths (`get_by_slug`, `get_by_source_and_slug`, `list_by_source`, `list_by_source_limited`) take `&LocaleCode`; new `list_slugs_with_locales_by_source` powers sitemap hreflang pairing. Ingestion reads `locale: <code>` from frontmatter and defaults to `en` when absent.
- **Global `SiteI18nConfig` on `WebConfig`** (`shared/provider-contracts/web_config/i18n.rs`). Declares `default_locale` + `supported_locales` and exposes a `locale_prefix()` helper (`""` for default locale, `/<code>` otherwise). Default-locale URLs keep the unprefixed shape (`/guides/foo`); non-default locales prefix the path (`/es/guides/foo`).
- **Locale-aware prerender pipeline.** `process_all_sources` fans out across `supported_locales × content_sources`; output paths are composed with `locale_prefix`. `PagePrepareContext` and `PageContext` carry `locale` and expose `with_locale()`; static-page prerenderers are invoked once per locale with locale-prefixed `output_path`. The per-row `locale` is injected into the template JSON so templates can render `<html lang>`. Missing translations are omitted entirely for that locale (no page, no sitemap entry, no hreflang alternate).
- **Sitemap hreflang alternates** (`SitemapUrlEntry.alternates`, `xml::SitemapUrlAlternate`). The generated `<urlset>` declares the `xhtml` namespace and each `<url>` emits one `<xhtml:link rel="alternate" hreflang="…"/>` per sibling locale plus an `x-default` link pointing at the default-locale URL.

### Removed

- `crates/domain/agent/src/{repository/content/{skill,agent}.rs, services/{skills/ingestion,agents/{ingestion,agent_entity}}.rs, models/{skill,agent}.rs, database_rows::AgentRow, schema/{agent_skills,agents}.sql}`
- `crates/app/sync/src/{local/{skills_sync,agents_sync},diff/{skills,agents},export/{skills,agents}}.rs` and the corresponding `pub use` re-exports (`SkillsLocalSync`, `AgentsLocalSync`, `AgentsDiffCalculator`, `AgentDiffItem`, `AgentsDiffResult`, `DiskAgent`).
- `crates/entry/cli/src/commands/{cloud/sync/{skills,agents}.rs, core/skills/{create,create_files,create_prompts,delete,edit,status,sync}.rs, core/plugins/generate/hooks.rs}`.
- `PluginConfig.hooks` and the corresponding `hooks_count` field on the CLI `plugin show` output.

## [0.8.0] - 2026-05-07

### Added

- **`GET /v1/bridge/whoami` (`routes/gateway/bridge_whoami.rs`).** Identity envelope for the bridge profile tab. Decodes the bearer JWT via `JwtContextExtractor::decode_for_gateway`, looks the user up via `UserRepository::find_by_id`, and returns `{user_id, email, display_name?, roles}` — only fields the gateway can authoritatively answer. The bridge consumer (`gui/handlers/profile.rs::identity_value`) tolerates the call failing or omitting fields and falls back to its locally verified identity snapshot for `tenant_id` / `provider`. Wired into `gateway_router` alongside the existing `/bridge/profile`, `/bridge/manifest`, and `/bridge/enabled-hosts` routes.
- **Per-user host enable preferences (`bridge_user_host_prefs`).** New table (schema in `crates/domain/oauth/schema/bridge_user_host_prefs.sql`) records which bridge-managed hosts (`claude-code`, `claude-desktop`, `cowork`, `codex-cli`) the user has enabled. `POST /v1/bridge/enabled-hosts` (`routes/gateway/bridge.rs::set_enabled_host`) upserts a row; `GET /v1/bridge/manifest` reads the rows and includes them as `enabled_hosts` in the signed manifest (when no rows exist, all known hosts default to enabled). Bridge-side `agents.json` is now derived from this manifest field on each apply, replacing the previous probe-based migration path.
- **Gateway protocol layer (`crates/entry/api/src/services/gateway/protocol/`).** Replaces the ad-hoc `converter.rs` / `flatten.rs` / `models.rs` / `upstream.rs` / `upstream/sse.rs` / `stream_tap/sse_parser.rs` files with a typed `CanonicalRequest`/`CanonicalResponse`/`CanonicalEvent` model and explicit inbound/outbound adapters. Inbound supports `anthropic_messages` and `openai_responses`; outbound supports `anthropic`, `openai_chat`, and `openai_responses`. Adapters register through `OutboundAdapterRegistration` for static dispatch. `stream_tap` is rewritten on top of the canonical event stream so per-provider SSE parsing no longer leaks into the safety/audit/usage layers.
- **Signed bridge manifest endpoint (`GET /v1/bridge/manifest`).** Returns a typed `SignedManifest` (moved from `bin/bridge/src/gateway/manifest.rs` to `crates/shared/models/src/bridge/`) populated from real data: skills via `SkillRepository::list_enabled`, agents via `AgentRepository::list_enabled`, plugins from on-disk `<system>/services/plugins/<id>/` walks (per-file sha256 + aggregate), managed MCP servers from `ServicesConfig.mcp_servers` filtered by `enabled`, revocations from `user_api_keys` rows where `revoked_at IS NOT NULL`, and `user` via `UserRepository::find_by_id`. Signed via `systemprompt_security::manifest_signing::sign_value` over a JCS canonical view that matches the bridge-side verifier byte-for-byte.
- **OAuth hook-token minting via `client_credentials`.** New `Permission::HookGovern` / `Permission::HookTrack` (hierarchy slot 15), `JwtAudience::Hook`, `JwtClaims.plugin_id`. New `systemprompt_security::auth::hook_token::HookTokenValidator` enforces signature + scope + `plugin_id` for `/api/public/hooks/{govern,track}`. Token endpoint accepts `plugin_id` and `audience` request fields via `ClientTokenOptions`; hook-scoped clients are pinned to `audience=hook`.
- **`POST /v1/bridge/oauth-client`** provisions or rotates the per-tenant OAuth client used for hook-token minting. Returns plaintext `client_secret` once at creation/rotation time. Backed by `provision_bridge_oauth_client` in `crates/domain/oauth/src/services/bridge.rs`.
- **Bridge heartbeat + active-device registry.**
  - New `bridge_sessions` table (`crates/domain/oauth/schema/bridge_sessions.sql`) keyed on `session_id`, with `bridge_version`, `os`, `hostname`, `started_at`, `last_heartbeat_at`, `last_activity_at`, and forwarded/token totals. Two indices on `last_heartbeat_at` for the active-devices query.
  - `BridgeSessionRepository` (`crates/domain/oauth/src/repository/bridge_session.rs`) — `upsert`, `list_active(within)`, `list_active_for_user`, `delete_stale`. All queries via compile-time `sqlx::query!` / `query_as!` macros.
  - `POST /v1/bridge/heartbeat` (`crates/entry/api/src/routes/gateway/bridge_heartbeat.rs`) — JWT-authed; typed `BridgeHeartbeatRequest`; upserts the session row and returns `204 No Content`.
  - Bridge polling loop (`bin/bridge/src/proxy/heartbeat.rs`) — 30 s cadence, spawned next to the existing token-refresh loop. Reuses the proxy's reqwest client and `TokenCache`. On `401` the token cache invalidates so the next tick re-authenticates.
  - `SessionContext::touch_activity()` is called on every successful messages-path forward, so `last_activity_at` reflects real inference traffic rather than just the heartbeat tick.
  - New CLI: `systemprompt admin bridge list [--user-id <id>] [--within-secs <N>]` (default 120 s = 4× heartbeat grace) for operators to list active devices.

### Changed

- **Breaking — `cowork` is renamed to `bridge` everywhere.** Clean cutover, no compatibility shims. A `0.7.x` bridge cannot authenticate against a `0.8.0` gateway and vice versa.
  - HTTP routes: `/v1/cowork/*` → `/v1/bridge/*`, `/v1/auth/cowork/*` → `/v1/auth/bridge/*`.
  - Wire formats: `JwtAudience::Cowork` (`"cowork"`) → `JwtAudience::Bridge` (`"bridge"`); `ClientId::cowork()` (`"sp_cowork"`) → `ClientId::bridge()` (`"sp_bridge"`); `SessionSource::Cowork` → `SessionSource::Bridge`.
  - DB: `cowork_exchange_codes` → `bridge_exchange_codes`. Idempotent `MIGRATION_002_RENAME_COWORK_TO_BRIDGE` added to the OAuth extension; existing deployments rename in place on next bootstrap.
  - Symbol renames across `systemprompt_oauth` (`issue_bridge_access`, `BridgeAuthResult`, `BridgeExchangeCode`, …), `bin/bridge` macros (`bridge_define_id!`, `bridge_define_token!`), and the file moves `services/cowork.rs` → `services/bridge.rs`, `routes/gateway/cowork.rs` → `routes/gateway/bridge.rs`, `commands/admin/cowork/` → `commands/admin/bridge/`.
  - Env vars: `SP_COWORK_*` → `SP_BRIDGE_*`. Config file: `~/.config/systemprompt/systemprompt-cowork.toml` → `systemprompt-bridge.toml`.
  - GitHub workflows, MDM templates, and `documentation/cowork/` → `documentation/bridge/` follow the same rename. Historical CHANGELOG entries are unchanged.
- **Marketplaces as first-class YAML-defined services.** Curated bundles of plugins, skills, MCP servers, and agents are now declared in YAML and validated at startup, mirroring the existing `PluginConfig` pattern.
  - New `MarketplaceConfig` model (`crates/shared/models/src/services/marketplace.rs`) with `MarketplaceConfigFile` wrapper and `MarketplaceVisibility` enum (`Public | Private | Org`). Aggregates plugins/skills/MCP servers/agents by reference only — never inlines them.
  - New typed `MarketplaceId` identifier (`crates/shared/identifiers/src/marketplace.rs`).
  - `ServicesConfig` gains a `marketplaces: HashMap<MarketplaceId, MarketplaceConfig>` field. `validate_marketplace_bindings()` resolves every `plugins.include`, `skills.include`, `mcp_servers`, and `agents.include` reference against the rest of the config and emits `ConfigValidationError::unknown_reference` on misses, so a typo in a marketplace YAML fails fast at startup.
  - Loader auto-discovers `<services>/marketplaces/<id>/config.yaml`, parses each as `MarketplaceConfigFile`, and inserts into `ServicesConfig.marketplaces` with duplicate detection (`ConfigLoadError::DuplicateMarketplace`). Inline declarations in includes also flow through `merge_no_dup`.
  - `Settings::default_marketplace_id: Option<String>` controls which marketplace `/marketplace.json` resolves to (fallback `"default"`).
  - API: `GET /marketplace.json` now serves the typed default marketplace; new `GET /marketplaces`, `GET /marketplaces/{id}`, `GET /marketplaces/{id}/manifest.yaml` for listing, resolved bundles, and raw YAML.
  - CLI: `systemprompt core plugins generate marketplace` is driven from `ServicesConfig.marketplaces` — emits `marketplace-<id>.json` per declared marketplace plus `marketplace.json` for the default.
- **Dynamic registration default for `token_endpoint_auth_method`.** `DynamicRegistrationRequest::get_token_endpoint_auth_method` now defaults to `client_secret_basic` per RFC 7591 §2 instead of returning `Result<_, String>`. Missing/empty values are accepted and defaulted instead of rejected with HTTP 400.
- **Dynamic registration `client_secret` + `registration_access_token` upgraded** from UUID-v4 strings (~122 bits of entropy) to 32-byte URL-safe random (~256 bits).

### Fixed

- **Gateway context-id is now guaranteed for every request, on every protocol.** Before this change, the bridge proxy only derived an `x-context-id` header for paths matching `/messages` or `/v1/messages` and only when the body parsed as Anthropic-shaped JSON; the gateway then hard-rejected anything that arrived without the header. OpenAI Responses traffic via `/responses` (Codex CLI, Gemini-shape clients) and any direct-to-gateway client therefore failed with `400 missing required x-context-id header`.
  - New shared module `systemprompt_models::gateway_hash` (FNV-1a 64-bit, length-prefixed, label-disambiguated) provides `conversation_prefix_hash` and `context_id_from_prefix_hash`. The bridge and the gateway compute the same `ContextId` for the same first turn, deterministically across processes.
  - Gateway `CanonicalRequest::derived_context_id()` flattens `system + first message` into the shared hash, so every inbound adapter (Anthropic Messages, OpenAI Responses, future shapes) gets identical derivation for free.
  - `routes/gateway/messages/extract.rs` switched from hard-fail `require_conversation_binding` to a header-or-derive policy: body parses first, then `x-context-id` is taken from the header if present, otherwise derived from canonical. The defence-in-depth invariant at `services/gateway/service.rs:39` is unchanged.
  - Bridge `proxy/forward.rs` no longer gates context derivation on path. `proxy/session.rs` `PrefixProbe` now recognises Anthropic Messages, OpenAI Chat Completions, and OpenAI Responses shapes and flattens array `content` parts. The bridge cache uses the same shared hash, so bridge-derived and gateway-derived ids never disagree for the same conversation.
  - Tests: `crates/tests/unit/shared/models/gateway_hash.rs` (17 tests covering hash determinism, FNV-1a segment-boundary disambiguation, role/system/content sensitivity, Unicode payloads, lowercase-hex `ctx_*` formatting, 1024-row collision-distribution sanity, and a frozen known-vector that locks the wire-format hash so future algorithm changes become an explicit breaking change) and `crates/tests/unit/bridge/proxy/derive_context_id.rs` (21 tests covering all three protocol shapes, the "second turn rehashes to same id" invariant, **cross-protocol equivalence** — Anthropic system + OpenAI-Chat leading-`role:"system"` message + OpenAI-Responses `instructions` all converge to the same `ContextId` for the same conversation — array-content concatenation, multi-system-message concatenation, default role inference, and resilience to extra unknown fields).
- **Bridge probe extracts inline OpenAI-Chat system messages.** `bin/bridge/src/proxy/session.rs` `PrefixProbe::first_turn` now coalesces leading `role:"system"` messages into the canonical `system` text, so an Anthropic body `{system, messages:[user]}` and an OpenAI Chat body `{messages:[system, user]}` carrying the same conversation hash to the same `ContextId`.
- **`profile_gateway` test crate compiles again.** `GatewayRoute` recently gained a `pricing: Option<ModelPricing>` field; the test fixtures in `crates/tests/unit/shared/models/src/profile_gateway.rs` were missing the initializer and broke the `systemprompt-models-tests` build. Added `pricing: None` to the two literal `GatewayRoute` constructors so the test crate compiles and `gateway_hash` tests can run.
- **`otel.rs` clippy hygiene.** Folded redundant `map(...).unwrap_or(...)` over `Option` into `map_or`, made `severity_to_level` a `const fn`, switched `&Option<AnyValue>` parameters to `Option<&AnyValue>` (Clippy `ref_option`), and dropped the now-unnecessary `#[allow(clippy::ptr_arg)]`. No behavior change.

## [0.7.0] - 2026-05-06

### Added

**Unified authorization decision plane (`crates/infra/security/src/authz/`)**

- **`AuthzDecisionHook` async trait** — single extension point for both the gateway `/v1/messages` proxy and the MCP RBAC middleware. Both enforcement sites call `evaluate(AuthzRequest) -> AuthzDecision` via a process-global slot installed at server startup.
- **`WebhookHook`** — fail-closed production implementation. POSTs `AuthzRequest` to an extension HTTP endpoint (e.g. the template's `POST /govern/authz`). Any transport error, non-2xx response, decode failure, or timeout denies the request and records the fault to the audit sink. There is no fail-open mode.
- **`DenyAllHook`** — bootstrap default and `mode: disabled` implementation. Denies every request and records to the audit sink so outages remain observable.
- **`AllowAllHook`** — dev/test only. Installed only when the operator passes the exact `unrestricted` acknowledgement in the profile; bootstrap fails otherwise. Every call logs an `ERROR` line and writes an audit row so unrestricted operation is never silent.
- **`AccessControlRepository`** — typed queries against `access_control_rules` (`list_rules_for_entity`, `list_rules_bulk`, `upsert_rule`, `delete_rule`, `set_default_included`, `get_default_included`). Generic over `EntityKind`.
- **`resolve(rules, user_id, roles, department, default_included) -> Decision`** — pure deny-overrides resolver with user > role > department > default specificity. Zero DB calls; suitable for unit testing.
- **`EntityKind` enum** (`GatewayRoute`, `McpServer`) — typed entity references in `AuthzRequest`; serializes to `"gateway_route"` / `"mcp_server"` for JSON compatibility with the extension webhook.
- **`GovernanceDecisionRepository` and `DbAuditSink`** — write every authorization decision (allow and deny) to the `governance_decisions` table with `entity_type`, `entity_id`, `user_id`, `tenant_id`, `decision`, and `evaluated_rules`. `NullAuditSink` for tests and pre-database bootstrap.
- **`install_from_governance_config`** — reads `services/governance/config.yaml` (`mode: webhook | disabled | unrestricted`) and installs the process-global hook at startup. Called from `AppContextBuilder::build` after the database pool is created.
- **Schema migrations** embedded via `AuthzExtension`: `access_control_rules` (entity × rule_type × access with deny-overrides precedence) and `governance_decisions` (unified audit log for all authorization decisions).
- **`systemprompt-security-authz-tests` crate** (`crates/tests/unit/infra/security/authz/`) — bootstrap, hook-runtime, webhook-hook, and profile-governance unit tests.

**JWT and profile changes**

- **`JwtClaims.department: Option<String>`** and **`JwtClaims.tenant_id: Option<TenantId>`** — new optional claims skipped during serialization when absent. Populated by the token issuer at login; forwarded to `AuthzRequest` at both enforcement sites without a DB round-trip per request.
- **`GovernanceConfig` and `AuthzMode`** profile types (`crates/shared/models/src/profile/governance.rs`). `AuthzMode` is `webhook | disabled | unrestricted`; `UNRESTRICTED_ACKNOWLEDGEMENT` is the sentinel string that must be set exactly for `AllowAllHook` to install.
- **Stable `id` field on `GatewayRouteView`** (`crates/shared/models/src/profile/gateway.rs`) — slug+hash ID persisted in `profile.yaml`; backfill keeps legacy profiles working without migration.

**External-agent catalog**

- **`ExternalAgentConfig` and `ExternalAgentKind`** types (`crates/shared/models/src/services/external_agent.rs`). Catalog entry for native apps and CLI tools that connect via the bridge binary (Claude Desktop, Codex CLI, Claude Code). Intentionally distinct from `AgentConfig` (server-side A2A agents).
- **`ExternalAgentId`** typed identifier (`crates/shared/identifiers/`).
- **`external_agents` field** wired through `ConfigLoader` (`RootConfig`, `PartialServicesFile`, merge logic) with a `DuplicateExternalAgent` error on name collision across included service files.

### Changed

- **`/v1/messages` gateway enforcement** (`crates/entry/api/src/routes/gateway/messages/extract.rs`): `extract_request_context` refactored into `read_gateway_body` and `build_authz_request` (≤58 lines each); missing `tenant_id` in the JWT now returns 401 instead of constructing an empty `TenantId`; `AuthzDecisionHook::evaluate` is called after JWT/scope validation via `global_hook()`; requests are explicitly denied when no hook is installed.
- **MCP RBAC middleware** (`crates/domain/mcp/src/middleware/rbac.rs`): missing `tenant_id` returns an authz-deny `McpError`; uses typed `EntityKind::McpServer`; `AuthzDecisionHook::evaluate` called after `enforce_rbac_from_registry` succeeds; explicitly denies when no hook is installed.

### Removed

- **`just check-bans` and `just check-bans-crate` recipes** (`justfile`) and the matching `check-bans` job in `.github/workflows/quality.yml`. The recipes were grep-based stand-ins for three rules: raw `String` ID fields, `*Manager` type names, and out-of-allowlist `sqlx::query()`. Typed-ID discipline and the `*Manager` preference remain reviewer-enforced conventions (already documented as such in `CLAUDE.md` and `instructions/prompt/rust.md`); the sqlx allowlist is enforced by clippy and `ci/check-sqlx.sh`. Dropping the recipes removes a governance surface that was producing busywork (23 historical `*Manager` flags across MCP/scheduler/agent internals) without a corresponding bug class. Existing dated audit reports under `instructions/audits/` continue to reference these recipes as historical evidence and are intentionally left unchanged.

## [0.6.0] - 2026-05-05

### Changed

- **Breaking — `DatabaseProvider`, `DatabaseTransaction`, and `DatabaseProviderExt` traits return `DatabaseResult<T>`** (`crates/infra/database/src/services/provider.rs`, `crates/infra/database/src/models/transaction.rs`). Every method that previously returned `anyhow::Result<T>` now returns `Result<T, RepositoryError>`. External crates implementing these traits must update return types and convert their backend errors via `RepositoryError::Database(#[from] sqlx::Error)`, `RepositoryError::Serialization(#[from] serde_json::Error)`, or `RepositoryError::invalid_state` for runtime invariant failures. Migration:
  ```rust
  // before
  async fn execute(&self, ...) -> anyhow::Result<u64> { ... }
  // after
  async fn execute(&self, ...) -> systemprompt_database::DatabaseResult<u64> { ... }
  ```
- **Breaking — `FromDatabaseRow::from_postgres_row` returns `DatabaseResult<Self>`** (`crates/infra/database/src/models/query.rs`). Decoders implementing the trait must return `Result<Self, RepositoryError>` instead of `anyhow::Result<Self>`.
- **Breaking — `Database::new_postgres`, `Database::from_config`, `Database::pool_arc`, `Database::write_pool_arc`, `Database::read_pool_arc`, `Database::begin`, and `PostgresProvider::new`** all return `DatabaseResult<T>` (`crates/infra/database/src/services/database.rs`, `crates/infra/database/src/services/postgres/mod.rs`).

### Added

- **`RepositoryError::InvalidState(String)` variant** plus `RepositoryError::invalid_state(msg)` constructor (`crates/infra/database/src/error.rs`). Captures driver-protocol invariant failures previously wrapped in `anyhow!` (transaction reused after commit, scalar query with no columns, unsupported `DbValue` type).
- **`From<systemprompt_database::RepositoryError> for systemprompt_traits::RepositoryError`** bridge so domain repositories that store the boxed-error variant pick up the typed database error transparently through `?`.
- **`#[from] systemprompt_database::RepositoryError` variants** added to `McpDomainError`, `OauthError`, `UserError`, `FilesError`, and `LoggingError`. Repositories propagating database errors via `?` no longer need a manual `.map_err(...)`.
- **Typed identifiers extended for cloud surfaces** — `TenantId`, `PriceId`, `TransactionId`, `CheckoutSessionId`, `ConnectionId`, `SectionId` now used end-to-end across `crates/infra/cloud/`, `crates/entry/cli/src/commands/cloud/`, and `crates/shared/models/src/api/cloud/**`. Eliminates 50+ raw-`String` ID call sites.
- **`domain_error!` declarative macro** (`crates/shared/models/src/errors/macros.rs`). Domain crates compose their typed error enum from a `common: [repository, io, json, yaml, validation, not_found, config, anyhow, http]` token list plus their own variants. Drops ~300 lines of boilerplate across `files`, `mcp`, etc.
- **`crates/shared/identifiers/src/{cloud,connection,section}.rs`** — new typed-ID modules backing the cloud and dashboard surfaces.

### Removed

- **`impl From<anyhow::Error> for RepositoryError`** legacy bridge (`crates/infra/database/src/error.rs`). The bridge was only required while the trait surface returned `anyhow::Result`; now obsolete.
- **`impl From<anyhow::Error> for UserError`** and **`impl From<anyhow::Error> for LoggingError`** — the trait surface no longer produces `anyhow::Error` to be absorbed.

### Quality

- `cargo check --workspace`: clean.
- `cargo clippy --workspace --all-targets -- -D warnings`: clean.
- `cargo test --manifest-path crates/tests/Cargo.toml --workspace`: **3578 passed, 0 failed.**
- `cargo sqlx prepare --workspace`: refreshed; `.sqlx/` cache committed.
- **CLAUDE.md** updated to point at canonical `instructions/prompt/rust.md` and to spell out the real comment policy: inline `//` only for non-obvious *why*, `///` not applied mechanically, `//!` on `lib.rs` and significant `pub mod` heads as the load-bearing form, banned in `entry/*` binaries and inside `crates/tests/**`.
- **`rust-coding-standards` skill cache** synced from marketplace source so it no longer says "delete `///`".
- **Lint hygiene** — every hand-written `#[allow(...)]` outside `crates/tests/` (54 sites) now carries a `// reason: ...` comment so external scanners can see the suppression rationale. No allow was removed; no behavior changed.
- **Sqlx allowlist documented** — extended the `sqlx::query(_)` allowlist in `CLAUDE.md` and `justfile` (`check-bans`) to cover `crates/entry/cli/src/commands/admin/setup/**` (bootstrap DDL: `CREATE USER` / `CREATE DATABASE` / `GRANT` / `CREATE EXTENSION`, which cannot bind identifier parameters and run before the target database exists). Each call site now carries an `// allowlist: bootstrap DDL` annotation.

## [0.5.0] - 2026-05-04

### Added

- **`AppPaths` accessor on `AppContext`** (`crates/app/runtime/src/context.rs`). `ctx.app_paths()` returns `&AppPaths` and `ctx.app_paths_arc()` returns `Arc<AppPaths>`. Replaces the deleted `AppPaths::get()` global singleton.
- **`OauthResult<T>` and `FilesResult<T>`** type aliases now exposed by `systemprompt-oauth` and `systemprompt-files` crates. Public-API surface (repositories, services, validators) returns these typed results.
- **`McpDomainResult<T>` and `AgentResult<T>`** type aliases on `systemprompt-mcp` and `systemprompt-agent`. Public-API surface (`McpServerRegistry`, `RegistryManager`, `LifecycleManager`, `ProcessManager`, `DatabaseManager`, `McpOrchestrator`, `AgentRegistry`, `AgentLifecycle`, `validate_agent_binary`) now returns the typed aliases. `McpDomainError` is the public name; `pub use rmcp::ErrorData as McpError` retains the existing `McpError` symbol for tool-call boundary use.
- **`systemprompt_config::load_profile_with_catalog`** — single entry point for loading a profile YAML from disk and resolving its gateway catalog. Lives in `crates/infra/config/src/profile_loader.rs` (with companion `profile_gateway::resolve_catalog`).
- **`crates/infra/config/src/bootstrap/`** module — new home for `SecretsBootstrap`, `ProfileBootstrap`, `manifest_seed`, and the `BootstrapSequence<S>` machinery. The `BootstrapSequence` is now `Uninitialized → ProfileInitialized → SecretsInitialized → BootstrapComplete` (paths state removed).
- **`CategoryIdUpdate` re-export** from `systemprompt-content` for explicit `Unchanged | Clear | Set(CategoryId)` semantics; replaces `Option<Option<CategoryId>>` in the CLI content-edit state.

### Changed

- **Breaking — `AppPaths` is no longer a global singleton.** `AppPaths::init` and `AppPaths::get` are deleted. `AppPaths::from_profile(&profile.paths)` is the sole constructor. Components that previously called `AppPaths::get()` now receive `&AppPaths` (or `Arc<AppPaths>`) explicitly: 42 call sites across `infra/`, `domain/`, `app/`, `entry/`, and `crates/tests/` were rewritten. `JobContext` carries `app_paths` as a type-erased `Arc<dyn Any + Send + Sync>` (parallel to `db_pool` and `app_context`) so generator/sync jobs can downcast without depending on `systemprompt-runtime`.
- **Breaking — bootstrap I/O moved out of `systemprompt-models`.** `SecretsBootstrap`, `ProfileBootstrap`, `manifest_seed`, and the `BootstrapSequence<S>` machinery now live in `systemprompt-config`. `Secrets::load_from_path` is replaced by free function `systemprompt_config::load_secrets_from_path`. `Config::try_init` / `Config::init` / `Config::from_profile` are replaced by `systemprompt_config::{try_init_config, init_config, init_config_from_profile, build_from_profile}`. `Config::is_initialized` / `Config::get` / `Config::install` remain on the type. `validators::skills::SkillConfigValidator` moves to `systemprompt_config::SkillConfigValidator`. ~110 import sites updated; 14 crates picked up a `systemprompt-config` dependency. Restores the `crates/shared/models/` "no I/O" invariant from `boundaries.md`.
- **Breaking — public APIs in `systemprompt-oauth` and `systemprompt-files` return typed `Result`.** `OAuthRepository::*`, `validate_jwt_token`, `SessionCreationService::create_anonymous_session` return `OauthResult<T>`. `FileRepository::*`, `FileService::*`, `AiService::*`, `ContentService::*` (in files crate), and `FilesAiPersistenceProvider::new` return `FilesResult<T>`. `#[from] sqlx::Error`, `#[from] anyhow::Error`, and `#[from] std::io::Error` adapters provide compatibility for internal helpers that still return `anyhow::Result`.
- **Breaking — public APIs in `systemprompt-mcp` and `systemprompt-agent` return typed `Result`.** Registry, lifecycle, process, database, and orchestrator surface methods now return `McpDomainResult<T>` / `AgentResult<T>`. Internal helpers and upstream trait impls (`McpRegistryProvider`, `AgentRegistryProvider`) keep `anyhow::Result`; `#[from] anyhow::Error` adapter bridges the boundary.
- **Breaking — `Profile::parse` removed; replaced with `Profile::from_yaml`.** `from_yaml` does pure YAML deserialization with no I/O. Gateway catalog resolution moved to `systemprompt_config::profile_gateway::resolve_catalog`. The single user-facing entry point is `systemprompt_config::load_profile_with_catalog(path)`. Restores the `crates/shared/models/` "no I/O" invariant for the profile module.
- **`bin/bridge` pins `systemprompt-identifiers = "0.5.0"`** with `path` override, so bridge resolves cleanly both locally and from crates.io once 0.5.0 ships.
- **`ProxyError::AuthChallenge(Box<Response<Body>>)`** — variant now boxes the `axum::Response` to satisfy `clippy::result_large_err`. Internal-only change; constructor now wraps with `Box::new`.

### Removed

- **Breaking — `AppPaths::get()` and `AppPaths::init`** from `crates/shared/models/src/paths/mod.rs`. Use `AppPaths::from_profile` and pass the value through `AppContext` or function arguments.
- **`PathError::NotInitialized` and `PathError::AlreadyInitialized`** variants — the singleton states they described no longer exist.
- **`BootstrapSequence::with_paths`, `with_paths_config`, `skip_paths`, `presets::full`, `PathsInitialized`** — paths are now built from the profile in the `AppContext` builder; no separate bootstrap step.
- **Re-exports of `SecretsBootstrap`, `ProfileBootstrap`, and `manifest_seed`** from `systemprompt-models`. Import from `systemprompt-config` instead.

### Quality

- `cargo clippy --workspace -- -D warnings`: clean (eliminated 12 pre-existing pedantic lints in CLI and proxy code: `result_large_err`, `option_if_let_else`, `needless_pass_by_value`, `option_option`, `assigning_clones`, `bool_to_int_with_if`, `manual_let_else`, `needless_borrow`). Closed 3 remaining lints in `systemprompt-test-mocks` (`type_complexity` x2, `derivable_impls` x1).
- `cargo test --manifest-path crates/tests/Cargo.toml --workspace`: **8984 passed, 0 failed, 0 ignored.** Repaired bridge-* test crates (async migration, `Cell` → `Mutex` for `Send + Sync`, `ureq` → `reqwest` mock construction, removed-module deletions, env-var renames). Updated migration-weight assertions to match the v0.4.4 weight re-spacing. `events-tests` and `concurrency-tests` migrated to bounded `mpsc::channel(SSE_BUFFER)`.

## [0.4.4] - 2026-05-03

### Added

- **Code-quality remediation pass** addressing findings from the v0.4.3 ruthless review:
  - **Granular facade features** in `systemprompt/Cargo.toml` — `logging`, `config`, `loader`, `events`, `client`, `security` are now individually selectable instead of being bundled only under `full`. Backwards-compatible: `full` still enables them all.
  - **`OauthError` and `FilesError` thiserror enums** (`crates/domain/oauth/src/error.rs`, `crates/domain/files/src/error.rs`) with `#[from] sqlx::Error`, `#[from] anyhow::Error`, and `#[from] std::io::Error` conversions. Public APIs can now expose typed errors at boundaries instead of opaque anyhow strings; existing internal anyhow remains and migrates incrementally.
  - **Migration weight headroom** — extension `migration_weight()` values re-spaced ×10 (database 1→10, users 10→100, scheduler 55→550, etc.). Reserved ranges going forward: 0–99 infra core, 100–199 shared platform, 200–999 domain, 1000+ third-party extensions.
- `crates/entry/api/src/services/gateway/captures.rs` — leaf module exposing `CapturedToolUse` and `CapturedUsage` so `audit.rs` and `parse.rs` no longer import each other.
- `crates/entry/cli/src/commands/admin/setup/common.rs` — leaf module with `PostgresConfig`, `generate_password`, `detect_postgresql`, `test_connection`, `enable_extensions`. Removes the back-edge from `postgres.rs` to `docker.rs`.
- `bin/bridge/src/gui/emit.rs` — leaf module with all `emit_*`, `send_emit`, and `send_reply*` helpers. Breaks the `command.rs ↔ ipc_runtime.rs` cycle.
- `.sentrux/rules.toml` and `.sentrux/baseline.json` — structural-quality gates for future agent sessions (`sentrux check` / `sentrux gate`).

### Changed

- **Refactor — bridge GUI command dispatcher** (`bin/bridge/src/gui/command.rs::dispatch`, cc 61 → ≤25). Split the 25-arm string match into family routers (`meta`, `gateway`, `auth`, `sync`, `host`, `agent`, `diagnostics`) chained via `Option<CommandOutcome>`.
- **Refactor — bridge GUI event dispatcher** (`bin/bridge/src/gui/dispatch.rs::dispatch`, cc 32 → ≤10). Split into `dispatch_window`, `dispatch_request`, `dispatch_finished`, `dispatch_lifecycle`, `dispatch_ipc` chained by `Result<(), UiEvent>`.
- **Refactor — bridge GUI event-kind tracer** (`bin/bridge/src/gui/dispatch.rs::event_kind`, cc 30 → ≤10). Bucketised into `request_kind`, `finish_kind`, `lifecycle_kind`, `ipc_kind`.
- **Refactor — startup-event renderer** (`crates/entry/cli/src/presentation/renderer.rs::handle_event`, cc 32 → ≤10). Split into `handle_phase_event`, `handle_service_event`, `handle_status_event`, `handle_terminal_event`.
- **Refactor — proxy auth validator** (`crates/entry/api/src/services/proxy/auth.rs::validate`, cc 33 → ≤8). Extracted `lookup_oauth_requirement`, `resource_path_for`, `mcp_session_fallback`, `challenge_or_error`, `ensure_required_scopes`.
- **Refactor — agent edit CLI** (`crates/entry/cli/src/commands/admin/agents/edit.rs::execute`, cc 37 → ≤6). Field-update logic moved to `apply_enabled_flags`, `apply_runtime_fields`, `apply_card_fields`, `apply_capability_fields`, `apply_metadata_fields`, `apply_mcp_server_changes`, `apply_skill_changes`, `apply_set_value_changes`.
- **Refactor — content-types edit CLI** (`crates/entry/cli/src/commands/web/content_types/edit.rs::execute`, cc 30 → ≤6). Extracted `apply_basic_flags`, `apply_sitemap_flags`, `apply_set_value_changes`, `apply_set_key`, `apply_sitemap_set`.
- **Refactor — content edit CLI** (`crates/entry/cli/src/commands/core/content/edit.rs::execute_with_pool`, cc 28 → ≤6). Introduced `ContentEditState` builder and per-field appliers.
- **Refactor — services cleanup CLI** (`crates/entry/cli/src/commands/infrastructure/services/cleanup.rs::execute`, cc 26 → ≤8). Extracted `no_services_result`, `dry_run_result`, `stop_running_services`, `stop_api_server`, `format_cleanup_message`.
- **Refactor — cloud status CLI** (`crates/entry/cli/src/commands/cloud/status.rs::execute`, cc 38 → ≤8). Split into `load_profile_info`, `load_credentials_and_tenants`, `render_status`, `render_profile`, `render_credentials`.
- **Refactor — keyword-table conversions**. Replaced six long if-else / match chains with static lookup slices: `parse_browser` / `parse_os` (`user_agent.rs` cc 44 → ≤4), `Validator::get_extension` (`upload/validator.rs` cc 43 → ≤3), `is_scanner_agent` (`scanner.rs` cc 41 → ≤6), `detect_mime_type` (`core/files/upload.rs` cc 35 → ≤3), `filter_log_events` (`ai_trace_display.rs` cc 26 → ≤6).

### Fixed

- 3 structural import cycles eliminated (gateway audit↔parse, setup docker↔postgres, bridge command↔ipc_runtime). 6 → 3 cycles reported by Sentrux; the remaining 3 (gemini params↔tools, gateway extract↔webauthn authenticate, bridge auth↔gateway_probe) are tree-sitter resolver false positives — neither file imports back from the other.

### Quality

- Sentrux structural-quality score: **5299 → 5935**, `sentrux check ✓ All rules pass` (`max_cycles=3`, `max_cc=38`, `no_god_files=false`).
- 16 functions exceeded cc=25 before; only `bin/bridge/web/js/components/sp-host-card.js::render` (cc=38) remains, intrinsic to its multi-state HTML template.

## [0.4.3] - 2026-04-29

### Added

- `JwtAudience::Cowork` variant in `crates/shared/models/src/auth/enums.rs` (`as_str` and `FromStr` covered).
- `SecretsBootstrap::manifest_signing_secret_seed() -> Result<[u8; 32], _>` accessor in `crates/shared/models/src/secrets_bootstrap.rs`.
- `manifest_signing::sign_value<T: Serialize>` and `canonicalize<T>` in `crates/infra/security` for RFC 8785 (JCS) canonical JSON.
- `systemprompt admin cowork rotate-signing-key` CLI generates a fresh ed25519 seed, persists it, and prints the resulting base64 pubkey.

### Changed

- **Breaking**: `issue_cowork_access_with` (`crates/domain/oauth/src/services/cowork.rs`) mints `audience: vec![JwtAudience::Cowork]` instead of `JwtAudience::Api`. A cowork JWT no longer authorises generic API endpoints.
- Manifest signing seed is now a dedicated 32-byte value persisted under `manifest_signing_secret_seed` in the secrets file, generated by `OsRng` on first bootstrap. Replaces the prior `Sha256(DOMAIN_SEPARATOR || jwt_secret)` derivation in `crates/infra/security/src/manifest_signing.rs::signing_key`. JWT HMAC compromise no longer compromises manifest signatures.

### Fixed

- `Secrets::parse` (`crates/shared/models/src/secrets.rs`) strips JSON `null` values from the root object before deserialization. Previously a literal `"openai": null` / `"gemini": null` failed `serde_json::from_str` with `invalid type: null, expected a string`, which the bootstrap path swallowed and fell back to env-loading with a `None` seed.
- Subprocesses spawned with `SYSTEMPROMPT_SUBPROCESS=1` no longer rotate the manifest signing seed on each launch. `crates/domain/agent/src/services/agent_orchestration/process.rs` and `crates/domain/mcp/src/services/process/spawner.rs` propagate `MANIFEST_SIGNING_SECRET_SEED` from the parent's loaded `Secrets` into the spawn env. `secrets_bootstrap.rs::ensure_manifest_signing_seed` `bail!`s under `SYSTEMPROMPT_SUBPROCESS=1` with no seed in env.

### Security

- Manifest signatures use RFC 8785 (JCS) canonical JSON. Signer and verifier produce byte-identical canonical output.
- Cowork JWTs are minted with `audience: Cowork`, distinct from API tokens. Cross-audience misuse is rejected at validation.

### Removed

- **Breaking**: `DOMAIN_SEPARATOR` constant and the `Sha256(DOMAIN_SEPARATOR || jwt_secret)` derivation path in `crates/infra/security/src/manifest_signing.rs`.

### Internal

- `serde_jcs = "0.1"` added to `crates/infra/security/Cargo.toml`.
- Workspace `sha2` added to `crates/shared/models/Cargo.toml`.

## [0.4.0] - 2026-04-24

### Security

- **Fly.io cloud credentials now fail closed on API validation error** (`crates/infra/cloud/src/credentials_bootstrap.rs`). Previously, `CredentialsBootstrap::init()` demoted a validation error to `tracing::warn!` on Fly.io and continued with unvalidated credentials, so expired/revoked tokens only surfaced at the first downstream API call. Now propagates `CredentialsBootstrapError::ApiValidationFailed` unless the operator opts into fail-open behaviour with `SYSTEMPROMPT_ALLOW_UNVALIDATED_CREDS=1`. Non-Fly.io paths already failed closed and are unchanged.

- **Tarball extraction in `systemprompt-sync` hardened against path traversal** (`crates/app/sync/src/file_bundler.rs`). `extract_tarball` and `extract_tarball_selective` now reject symlinks and hard links, absolute paths, and any path containing `..`; enforce that the first path component is in the `INCLUDE_DIRS` allowlist (`agents`, `skills`, `content`, `web`, `config`, `profiles`, `plugins`, `hooks`); and canonicalise the destination parent, rejecting the entry if it escapes the target directory. Both entry points now funnel through a single `extract_tarball_filtered` helper. New `SyncError::TarballUnsafe(String)` variant. Pair with the equivalent hardening already in `crates/entry/api/src/routes/sync/files.rs`.

- **Auth middleware renamed to reflect its advisory role, `RequireAuth` extractor added** (`crates/entry/api/src/services/middleware/auth.rs`). `auth_middleware` → `auth_enrichment_middleware` and `AuthMiddleware::apply_auth_layer` → `apply_auth_enrichment_layer`. The middleware only attaches `Extension<AuthenticatedUser>` on successful JWT extraction and never rejects requests — enforcement lives in `ContextMiddleware`. New `RequireAuth(pub AuthenticatedUser)` extractor with `FromRequestParts` impl returns `401 Unauthorized` when the extension is absent, giving handlers a compile-time-checked auth primitive independent of `ContextMiddleware`. Neither the old function nor `apply_auth_layer` had external callers, so no downstream churn.

### Breaking

- **Removed `systemprompt::prelude::{Entity, EntityId, GenericRepository, RepositoryExt}`** (#5). The generic repository composed SQL at runtime from `E::TABLE`/`E::COLUMNS` and cannot satisfy the project's MANDATORY "SQLX macros only" rule (`query!` requires a string literal at compile time). No internal callers, no `impl Entity for` sites — the abstraction was dormant. Downstreams using the facade should migrate to per-entity repositories with `sqlx::query!()` / `query_as!()` (see `ServiceRepository`, `CleanupRepository` in `crates/infra/database/src/repository/` for the pattern). `crates/infra/database/src/repository/entity.rs` deleted.

- **`QueryExecutor::execute_query(sql, read_only)` replaced by `execute_readonly(sql, row_limit)` and `execute_write(sql)`** (#7). The old API passed a `bool` to switch modes; the new API encodes the mode in the entry point and returns the new `AdminSql` newtype's error variants if validation fails. Old callers using `executor.execute_query(sql, true)` become `executor.execute_readonly(sql, None)`; `executor.execute_query(sql, false)` becomes `executor.execute_write(sql)`.

### Changed

- **`DatabaseAdminService::{describe_table, get_table_indexes, count_rows}` now take `&SafeIdentifier` instead of `&str`** (#6). New `SafeIdentifier` newtype (exported from `systemprompt_database`) validates PostgreSQL identifiers at the boundary: 63-byte length cap, ASCII-letter-or-underscore lead, `[A-Za-z0-9_]` body only. Inline alphanumeric checks scattered across three admin methods removed; the invariant now rides the type. CLI callers (`db describe`, `db count`, `db indexes`) parse user input into a `SafeIdentifier` once at the CLI boundary and propagate it.

- **Admin SQL query executor hardened with `AdminSql` newtype and row cap** (#7). `AdminSql::parse_readonly(raw)` strips SQL line (`-- ...`) and block (`/* ... */`) comments, rejects multi-statement queries (any non-trailing `;`), requires a read-only prefix (`SELECT | WITH | EXPLAIN | SHOW | TABLE | VALUES`), and rejects forbidden keywords anywhere (drop, delete, insert, update, alter, create, truncate, grant, revoke, copy, vacuum, call, lock, set, reset, rename). Default row cap of 1000 on the read-only path, configurable per-call. Replaces the previous lowercase prefix + substring block-list, which missed comment-smuggled keywords and had no multi-statement guard.

### CI

- **New `ci/check-sqlx.sh` allowlist guard** (#8) fails if an unverified `sqlx::query*(...)` call appears outside a short list of structurally-dynamic sites (admin introspection, postgres driver, CLI bootstrap, integration test fixtures). Verified macros (`query!`, `query_as!`, `query_scalar!`) are unaffected. Wired into `just lint-sqlx` and `just style-check` step 4. Prevents regressions after this release tightens the unverified-query surface.

- **Regenerated per-crate `.sqlx/` offline caches** (#9) so `SQLX_OFFLINE=true cargo build --workspace` produces byte-identical output against the current live schema. Required for crates.io publishing.

## [0.3.2] - 2026-04-24

### Fixed

- **Static content route handler scoped slug lookup by `source_id`** (`crates/entry/api/src/services/static_content/static_files.rs`). `serve_static_content` extracted `(slug, source_id)` from the route matcher but discarded the source, then called `ContentRepository::get_by_slug(slug)` — a slug-only query. Any slug present in a different source (e.g. `about` as a page) caused `/guides/about`, `/documentation/about`, etc. to match a foreign record and return the "Content Not Prerendered" 500 page instead of 404. `source_id` is now threaded through `ContentPageRequest` and lookup uses `get_by_source_and_slug`.

- **Surface binary name and domain identifier on `Command::new` and `File::open` spawn errors across MCP, scheduler, sync, agent, and CLI paths.** The MCP port-manager reconciliation (`crates/domain/mcp/src/services/network/port_manager.rs`) shelled out to `lsof -ti :<port>` with bare `?` propagation. When `lsof` was missing from the runtime image, the ENOENT on `execve("lsof")` surfaced as a contextless `No such file or directory (os error 2)` and required `strace` to diagnose. Root fix is adding `lsof` to the runtime apt list, but the diagnosability gap is systemic: ~30% of `Command::new` sites discarded the binary name, args, and relevant identifier (port/pid/pattern/path) from the error path.

  Wrapped every flagged spawn site with `anyhow::Context::with_context` (or `tracing::warn!` where the return type is `Option`/`bool` and changing the signature would ripple through callers). Error messages now name the invocation (`failed to run \`lsof -ti :{port}\` for port {port}`) plus the domain identifier so operators don't have to re-derive context.

  Files touched: `crates/domain/mcp/src/services/network/port_manager.rs` (primary incident), `crates/domain/mcp/src/services/process/{pid_manager,cleanup,monitor,utils}.rs`, `crates/app/scheduler/src/services/orchestration/process_cleanup.rs` (previously silent `.ok()?` / `.is_ok_and(...)` converted to logging on failure), `crates/domain/agent/src/services/agent_orchestration/{port_manager,process}.rs`, `crates/domain/agent/src/services/agent_orchestration/orchestrator/cleanup.rs`, `crates/entry/cli/src/commands/cloud/tenant/docker/database.rs` (7 `docker exec psql` sites), `crates/entry/cli/src/shared/docker.rs`, `crates/app/sync/src/crate_deploy.rs` (new `SyncError::CommandSpawnFailed` variant), `crates/app/sync/src/file_bundler.rs` (new `SyncError::FileOpenFailed` variant), `crates/entry/cli/src/commands/web/templates/show.rs`.

- **HTTP-client timeout literals scattered across ~15 sites consolidated into `systemprompt_models::net`.** Generic 30s / 10s / 5s timeouts were inlined as `Duration::from_secs(…)` literals across cloud API, sync API, CIMD fetcher, OAuth credentials verify, CLI session auth, shared `SystempromptClient`, MCP streaming client, proxy client pool, API health checker, agent TCP monitor, and the two image-gen providers — with a dead `TimeoutConfiguration` struct in `crates/domain/agent/src/services/shared/resilience.rs` trying (and failing) to be the source of truth. Introduced `crates/shared/models/src/net.rs` with twelve named `Duration` consts (`HTTP_CONNECT_TIMEOUT`, `HTTP_DEFAULT_TIMEOUT`, `HTTP_HEALTH_CHECK_TIMEOUT`, `HTTP_AUTH_VERIFY_TIMEOUT`, `HTTP_SYNC_DEPLOY_TIMEOUT`, `HTTP_STREAM_CONNECT_TIMEOUT`, `HTTP_KEEPALIVE`, `HTTP_POOL_IDLE_TIMEOUT`, `AGENT_MONITOR_TCP_TIMEOUT`, `AGENT_READINESS_TCP_TIMEOUT`, `IMAGE_GEN_LONG_POLL_TIMEOUT`, `IMAGE_GEN_OPENAI_TIMEOUT`) so intent is explicit where values diverge (e.g. 300s for long-poll image gen, 2s for aggressive readiness probes, 15s for agent-startup grace). All 15 sites now reference these consts; every timeout preserves its previous numeric value — no runtime-behaviour change. Dead `TimeoutConfiguration` / `TimeoutType` enum deleted.

- **Consolidated further duplicate literals and removed an orphaned `AgentExtension` module.** `https://api.systemprompt.io` was inlined twice in `crates/infra/cloud/src/credentials_bootstrap.rs` (shadowing the existing `constants::api::PRODUCTION_URL`); both now reference the const. The A2A artifact-rendering extension URI `https://systemprompt.io/extensions/artifact-rendering/v1` was duplicated across 4 files — extracted to `systemprompt_models::a2a::ARTIFACT_RENDERING_URI` and wired into `agent_card.rs`, `artifact_transformer/mod.rs`, and `batch_builders.rs`. A second parallel `AgentExtension` struct in `crates/shared/models/src/a2a/agent_extension.rs` was an orphan (not in `mod.rs`, not referenced) — deleted. Production/sandbox DB hostnames (`db.systemprompt.io`, `db-sandbox.systemprompt.io`) in `swap_to_external_host` promoted to `constants::api::DB_PRODUCTION_HOST` / `DB_SANDBOX_HOST` next to the existing URL consts. `CALLBACK_TIMEOUT_SECS = 300` was declared twice (`oauth` and `checkout` modules) — lifted to a single top-level const aliased by both. User-Agent strings in the CIMD fetcher and webhook delivery service had hardcoded version suffixes (`systemprompt.io-OS/2.0`, `systemprompt.io-Webhook/1.0`) — now use `concat!("…/", env!("CARGO_PKG_VERSION"))` so the UA always matches the running binary.

- **CLI `--version` and API discovery reported stale hardcoded versions; protocol-spec versions were duplicated as literals.** `crates/entry/cli/src/args.rs:80` pinned the clap `#[command(version = "0.1.0")]` attribute to a literal, so `systemprompt --version` returned `0.1.0` regardless of the workspace version or the release tag. Swapped to `env!("CARGO_PKG_VERSION")` which clap resolves at build time from the crate's inherited workspace version. Same fix applied to the API gateway discovery endpoint (`crates/entry/api/src/services/server/discovery.rs:18`, user-visible `/` response) and the plugin marketplace generator (`crates/entry/cli/src/commands/core/plugins/generate/marketplace.rs:60`). Extracted the A2A and MCP protocol-spec versions into named constants to eliminate duplicate literals: `systemprompt_agent::A2A_PROTOCOL_VERSION = "0.3.0"` (replaces duplicates at `crates/domain/agent/src/models/web/card_input.rs:31` and `crates/entry/cli/src/commands/admin/agents/create.rs:94`) and `systemprompt_mcp::MCP_PROTOCOL_VERSION = "2024-11-05"` (replaces duplicates at `crates/domain/mcp/src/services/registry/trait_impl.rs:87` and `crates/entry/api/src/routes/agent/registry.rs:127`). These are pinned to external protocol specs — not our crate version — so the const form preserves intent while killing drift risk.

## [0.3.1] - 2026-04-22

### Fixed

- **Gateway tracing — six bugs overstating cost ~130× and hiding every downstream observability surface.** End-to-end audit (documented in `cowork-tracing.md`) of a live minimax gateway request found that cost reporting and every CLI read path (`audit --full`, `trace list`, `trace show`, `analytics conversations list`) was broken for gateway traffic. Root fixes in dependency order:

  - **`AnthropicCompatibleUpstream` honours `upstream_model`** (`crates/entry/api/src/services/gateway/upstream.rs`). Previously forwarded the raw request body unchanged, sending the client's `claude-sonnet-4-6` string to minimax regardless of `route.upstream_model`. Now computes `ctx.route.effective_upstream_model(&ctx.request.model)`, rewrites `body.model` only if it differs (pass-through stays zero-copy), and captures `response.model` into a new `UpstreamOutcome::Buffered { served_model, .. }` field so the audit layer learns what minimax actually served.

  - **`ai_requests.model` now stores the served model, not the client request** (`crates/entry/api/src/routes/gateway/messages.rs`, `crates/entry/api/src/services/gateway/audit.rs`). `GatewayRequestContext.model` is seeded from `route.effective_upstream_model()` at handler entry. `GatewayAudit::set_served_model()` overwrites `ai_requests.model` via new `AiRequestRepository::update_model` when the upstream response's `model` field differs from the route guess. Streaming path captures this from the `message_start` SSE frame via `stream_tap`.

  - **Real minimax pricing + unreachable match arm removed** (`crates/entry/api/src/services/gateway/pricing.rs`). The previous minimax branch had two identical `ModelPricing { 0.2, 1.1 }` arms (dead pattern match) at rates ~130× actual MiniMax API pricing. Replaced with per-family rates (`minimax-text-01` / `abab6.5` at $0.0002/$0.0011 per 1k, `minimax-m1` / `abab7-chat-preview` at $0.0004/$0.0022). Unknown models now fall through to `unknown()` which logs a warning and returns zero cost — missing entries are loud instead of silently overbilling. Pricing lookup moved from `GatewayAudit::new()` to `GatewayAudit::complete()` so the served model drives the rate.

  - **`ai_request_messages` populated from gateway path** (`crates/entry/api/src/services/gateway/audit.rs`, `crates/entry/api/src/services/gateway/parse.rs`). `GatewayAudit::open()` now parses the `AnthropicGatewayRequest` and inserts each message (plus any `system` prompt at `sequence_number=0`) via `AiRequestRepository::insert_message`. New `flatten_system_prompt` / `flatten_message_content` helpers join text blocks and JSON-encode tool_use / tool_result blocks. `complete()` appends the assistant response via `add_response_message`, extracted by new `parse::extract_assistant_text`. `audit <id> --full` now shows the full conversation turn instead of `"messages": []`.

  - **Gateway traces visible in `trace list` / `trace show`** (`crates/infra/logging/src/trace/list_queries.rs`). The `require_tracked` filter required `status IS NOT NULL`, which comes from `agent_tasks` — gateway requests don't create task rows, so their traces were hidden unless `--include-system` was passed. Filter dropped; `exclude_system` still drops the literal `"system"` bucket. `trace show` already renders AI summary when log events are empty, so it surfaces gateway traces as soon as they're discoverable.

  - **Gateway sessions in `analytics conversations list`** (`crates/domain/analytics/src/repository/conversations.rs`). `list_conversations` was `user_contexts`-only, populated exclusively by the agent path. Query rewritten as UNION of two CTEs: the original `agent_convs` (unchanged semantics) and a new `gateway_convs` that synthesizes rows from `ai_requests` where `task_id IS NULL`, grouped by `session_id`, counting `ai_request_messages` (populated by the Bug 3 fix). A `NOT EXISTS` guard prevents double-counting sessions that also have a `user_contexts` row.

  Added new `AiRequestRepository::update_model(id, model)` method (`crates/domain/ai/src/repository/ai_requests/mutations.rs`).

### Changed

- **Gateway helpers extracted to `gateway::flatten`** (`crates/entry/api/src/services/gateway/flatten.rs`, new). Consolidates `flatten_system_prompt`, `flatten_message_content`, `rewrite_request_model` (body JSON substitution for Anthropic-compatible upstream), and `parse_served_model` (response-body model extraction) into one module. Keeps `audit.rs` and `upstream.rs` near the 300-line coding-standards cap and isolates the JSON-at-protocol-boundary surface. Audit `build_record`, `persist_request_messages`, `persist_tool_calls` split into dedicated methods for function-length discipline.

  Verification: `cargo check --workspace` + `cargo clippy --workspace --all-targets` clean with `-D warnings`; `cargo fmt --all -- --check` clean; `systemprompt-api-tests` (429 passing) and `systemprompt-logging-tests` green. Expected end-to-end behavior: a minimax request now records cost within ±5% of the real MiniMax invoice, `audit --full` shows the full conversation, and the trace/analytics CLI commands surface gateway traffic without flags.

---

## [0.3.0] - 2026-04-22

### Added

- **LLM Gateway — `/v1/messages` inference routing.** Organisations using Claude for Work (formerly Claude Cowork) can now set `api_external_url` in their fleet MDM configuration to `https://systemprompt.io` and have every Claude Desktop inference request flow through the gateway. The gateway:
  - Exposes `POST /v1/messages` at the Anthropic wire format — fully compatible with the Claude API SDK, Claude Desktop, and any Anthropic-SDK client.
  - Authenticates with a systemprompt JWT carried in the `x-api-key` header (falls back to `Authorization: Bearer`). No additional API key is issued; the organisation's existing user JWTs serve as the credential.
  - Routes requests to any configured upstream provider based on `model_pattern` rules in the profile YAML. Supported provider types: `anthropic`, `openai` (OpenAI-compatible), `moonshot` (Kimi), `qwen`, `gemini` (stub — not yet dispatched).
  - **Anthropic upstream**: transparent byte proxy. Raw request bytes forwarded verbatim to the upstream endpoint with the upstream API key substituted; the response stream is piped back unmodified. Preserves extended thinking blocks, cache-control headers, and all Anthropic-specific SSE events exactly.
  - **OpenAI-compatible upstream**: converts Anthropic request format → OpenAI `/v1/chat/completions` format, proxies to the upstream, converts the response back to Anthropic format. For streaming, maps OpenAI SSE delta events to Anthropic `message_start` / `content_block_start` / `content_block_delta` / `message_delta` / `message_stop` SSE frames.
  - **API key resolution**: upstream API keys are resolved from the existing secrets file by secret name (`api_key_secret` in the route config). No new credential storage mechanism.
  - **Conditional mount**: the `/v1` router is only registered when `gateway.enabled: true` in the active profile — zero overhead for deployments that don't use the gateway.

- **Gateway profile configuration schema.** New `gateway` block in profile YAML (all fields optional; block absent = gateway disabled):

  ```yaml
  gateway:
    enabled: true
    routes:
      - model_pattern: "claude-*"
        provider: anthropic
        endpoint: "https://api.anthropic.com/v1"
        api_key_secret: "anthropic_api_key"
      - model_pattern: "moonshot-*"
        provider: moonshot
        endpoint: "https://api.moonshot.cn/v1"
        api_key_secret: "kimi_api_key"
        upstream_model: "moonshot-v1-8k"   # optional: override model name sent upstream
      - model_pattern: "qwen-*"
        provider: qwen
        endpoint: "https://dashscope.aliyuncs.com/compatible-mode/v1"
        api_key_secret: "qwen_api_key"
      - model_pattern: "*"                  # fallback route
        provider: anthropic
        endpoint: "https://api.anthropic.com/v1"
        api_key_secret: "anthropic_api_key"
  ```

  Routes are evaluated in order; first `model_pattern` match wins. Patterns support `*` wildcard prefix/suffix matching. `extra_headers` map is available per route for provider-specific requirements.

- **`GatewayProvider::is_openai_compatible()`** — `const fn` on the provider enum; returns `true` for `OpenAI`, `Moonshot`, `Qwen`. Used internally to select the conversion path.

- **`GatewayRoute::find_route(model)`** — resolves the first matching route for a given model name from a `GatewayConfig`. Returns `None` if no route matches (handler returns 404).

- **`GatewayRoute::effective_upstream_model(model)`** — returns `upstream_model` if set, otherwise echoes the client-provided model name. Enables transparent model aliasing (e.g. client requests `moonshot-v1-8k`; gateway can remap to a different upstream model name without the client knowing).

- **`JwtContextExtractor::extract_for_gateway(jwt_token: &JwtToken)`** — new method on the JWT middleware extractor. Accepts a typed `JwtToken` identifier (not a raw `&str`), validates it, and returns a `RequestContext`. Used by the gateway handler to validate the `x-api-key` credential without relying on the standard `Authorization: Bearer` middleware layer.

- **`ApiPaths::GATEWAY_BASE`** constant — `/v1` path prefix for the gateway router.

- **Cowork credential-helper auth path.** Claude for Work clients configure a `Credential helper script` that prints a bearer token on stdout; core now ships the helper binary plus the matching gateway endpoints that exchange a lower-privilege credential for a short-lived JWT carrying canonical identity headers.

  Gateway endpoints (mounted under `/v1/gateway/auth/cowork/` when `gateway.enabled: true`):

  - `POST /pat` — `Authorization: Bearer <pat>` → verifies the PAT via `systemprompt_users::ApiKeyService::verify`, loads the user via `systemprompt_oauth::repository::OAuthRepository::get_authenticated_user`, returns `{token, ttl, headers}` with a fresh JWT and the canonical header map.
  - `POST /session` — stub returning `501` (dashboard-cookie exchange not yet wired).
  - `POST /mtls` — stub returning `501` (device-cert exchange not yet wired).
  - `GET /capabilities` — returns `{"modes":["pat"]}`; probes advertise which exchange modes the deployment accepts.

  The JWT-assembly + header map live in `systemprompt_oauth::services::cowork` (`issue_cowork_access`, `issue_cowork_access_with`, `CoworkAuthResult`) so the route handler in `entry/api` stays thin — it only extracts the bearer, verifies via `ApiKeyService`, and calls the oauth-domain service. Headers returned in the response body use core's canonical constants from `systemprompt_identifiers::headers::*` (`x-user-id`, `x-session-id`, `x-trace-id`, `x-client-id`, `x-tenant-id`, `x-policy-version`, `x-call-source`) so Cowork merges them into every subsequent `/v1/messages` call and the gateway middleware reads real identity on every request.

- **`systemprompt-cowork` credential helper + sync agent binary.** Standalone crate at `bin/cowork/` (excluded from the workspace so it does not compile during `cargo build --workspace` and does not land in the `systemprompt` crates.io package). Dependency footprint is deliberately minimal (`ureq` + `rustls` + `serde` + `toml` + `ed25519-dalek`) — no `tokio`, `sqlx`, or `axum`.

  - **Progressive capability ladder**: probes credential providers in descending strength (mTLS → dashboard session → PAT). First provider that returns a token wins; absent providers return `NotConfigured` and the chain falls through. No user-facing "pick a mode" step.
  - **Providers** (`src/providers/{mtls,session,pat}.rs`) share a single `AuthProvider` trait returning `Result<HelperOutput, AuthError>` where `AuthError::NotConfigured` silently advances the chain.
  - **Config**: TOML at `~/.config/systemprompt/systemprompt-cowork.toml` (or `$SP_COWORK_CONFIG`). All sections optional — absent sections mean the provider is skipped. Dev overrides: `$SP_COWORK_GATEWAY_URL`, `$SP_COWORK_PAT`, `$SP_COWORK_DEVICE_CERT`, `$SP_COWORK_USER_ASSERTION`.
  - **Cache**: signed JWT + expiry written to the OS cache dir with mode `0600` on unix. Cached token is emitted directly if valid; only on cache miss does the probe chain run.
  - **Stdout contract**: exactly one JSON object matching `{token, ttl, headers}` — Anthropic's `inferenceCredentialHelper` format. All diagnostics go to stderr. Exit 0 on success, non-zero on failure.
  - **Sync agent**: `install`, `sync`, `validate`, `uninstall` manage Cowork's `org-plugins/` mount (macOS `/Library/Application Support/Claude/org-plugins/`, Windows `C:\ProgramData\Claude\org-plugins\`, Linux `${XDG_DATA_HOME:-$HOME/.local/share}/Claude/org-plugins/`) — pulling signed plugin manifests and managed MCP allowlists from the gateway.
  - **Release cadence**: tagged `cowork-v*`; `.github/workflows/cowork-release.yml` builds binaries for `aarch64-apple-darwin`, `x86_64-apple-darwin`, `x86_64-pc-windows-msvc`, and `x86_64-unknown-linux-gnu`, attaches them to a GitHub Release with SHA256SUMS. Triggered only by the helper tag pattern; core's normal CI is untouched.
  - **Build targets**: `just build-cowork [target]` and `just build-cowork-all`.

- **`ClientId::cowork()`** constructor — returns `sp_cowork`, recognised as `ClientType::FirstParty` via the existing `sp_` prefix rule. Used by the Cowork JWT issuance path so every token issued to a Cowork session can be identified as first-party Cowork traffic in audit logs.

- **`SessionSource::Cowork`** variant + `SessionSource::from_client_id("sp_cowork") → Cowork`. Used as the `x-call-source` header value on Cowork-issued tokens so downstream middleware and analytics can distinguish Cowork sessions from Web / CLI / API / OAuth / MCP sessions.

- **`systemprompt_identifiers::PolicyVersion`** — new typed ID with `PolicyVersion::unversioned()` constructor. Exposed in the Cowork helper's header response as `x-policy-version` so a future policy-bundle-hash propagation feature plugs in without changing the wire contract.

- **`systemprompt_identifiers::headers::TENANT_ID` / `POLICY_VERSION`** — new canonical header constants (`x-tenant-id`, `x-policy-version`) alongside the existing `USER_ID`, `SESSION_ID`, `TRACE_ID`, `CLIENT_ID` family. All Cowork-issued tokens carry the full set in the response body's `headers` map.

- **Gateway provider registry — extensions can register custom upstreams.** `GatewayProvider` is no longer a closed enum; `GatewayRoute.provider` is now a free-form string tag resolved at dispatch time against a registry built at startup. Extension crates register new providers with:

  ```rust
  inventory::submit! {
      systemprompt_api::services::gateway::GatewayUpstreamRegistration {
          tag: "my-provider",
          factory: || std::sync::Arc::new(MyUpstream),
      }
  }
  ```

  The new `GatewayUpstream` trait (`async fn proxy(&self, ctx: UpstreamCtx<'_>)`) is the single integration seam. Built-in tags seeded automatically: `anthropic`, `minimax`, `openai`, `moonshot`, `qwen`. Extension-registered tags may shadow built-ins (logged as a warning).

- **MiniMax provider.** MiniMax ships an Anthropic-compatible endpoint at `https://api.minimax.io/anthropic`, so the new `minimax` tag reuses the Anthropic-compatible upstream verbatim — streaming, tool use, and `thinking` blocks pass through untouched. Example route:

  ```yaml
  gateway:
    enabled: true
    routes:
      - model_pattern: "MiniMax-*"
        provider: minimax
        endpoint: https://api.minimax.io/anthropic
        api_key_secret: minimax
  ```

  The `api_key_secret` resolves through `Secrets.custom`, so no changes to the secrets schema are required.

- **Gateway governance — full audit, policy, quota, and safety pipeline.** Every `/v1/messages` call now lands a structured audit trail, enforces tenant-scoped policy, and runs through a pluggable safety scanner before and after dispatch. This closes the product-level gap where the gateway proxied requests to MiniMax/Anthropic/OpenAI upstreams but persisted nothing beyond a one-line tracing log. For a platform whose core promise is "governance for all AI calls", this is the spine that makes the promise enforceable rather than aspirational.

  - **`ai_requests` persistence on the gateway path.** The handler mints a typed `AiRequestId` at ingress, writes a `pending` row before dispatch (with `user_id`, `tenant_id`, `session_id`, `trace_id`, `provider`, `model`, `max_tokens`, `is_streaming`), and updates it to `completed` with `input_tokens` / `output_tokens` / `cost_microdollars` / `latency_ms` once the upstream response resolves. Non-streaming responses parse the buffered JSON to extract usage + `tool_use` blocks; streaming responses run through an SSE tap (see below) that captures the same data without mutating the byte stream. On upstream error, the row flips to `failed` with `error_message` populated. Audit writes are best-effort — a DB outage logs an ERROR but never blocks the proxied request.

  - **`ai_request_payloads` table — full request/response retention.** New JSONB columns per `AiRequestId`: `request_body`, `response_body`, plus truncation flags + byte counts. 256 KB cap per side; overflow writes `NULL` for the body and a head+tail excerpt (`request_excerpt` / `response_excerpt`, 8 KB each side with a `...<truncated N bytes>...` marker). Response capture for streams reconstructs the full byte payload from the tap before persisting. Payload writes are fire-and-forget (`tokio::spawn`) so the client connection closes at upstream speed regardless of DB write latency.

  - **`ai_request_tool_calls` — `tool_use` capture + `tool_result_payload` column.** Every `tool_use` block in the response (Anthropic `content[].type == "tool_use"` for buffered JSON; `content_block_start` + `input_json_delta` accumulation for SSE) writes one row to `ai_request_tool_calls` with sequence number, `ai_tool_call_id`, `tool_name`, and `tool_input` (64 KB cap with truncation marker). New nullable `tool_result_payload JSONB` column is added to close the loop on follow-up turns — the migration is in place; the match-on-`ai_tool_call_id` upsert from the next request is plumbed for a follow-up iteration.

  - **`ai_safety_findings` table + pluggable `SafetyScanner` trait.** New async trait at `crates/entry/api/src/services/gateway/safety/` with two implementations: `HeuristicScanner` (known jailbreak prefixes → severity=medium; email regex → low; Luhn-valid 16-digit credit card → high) and `NullScanner` (for tests). Scanning runs pre-dispatch on the request and post-dispatch on the response (per-chunk SSE scanning is wired but currently reuses the final-buffered path). Findings persist with phase (`request` / `response`), severity, category, and an excerpt. Current release is warn-only — findings land in the table and can be queried, but don't short-circuit the request. The policy `safety.block_categories` field is plumbed to the dispatch path and gates a `451` short-circuit in the next iteration.

  - **`ai_quota_buckets` table + token-bucket enforcement.** Per-`(tenant_id, user_id, window_seconds, window_start)` atomic counters via `INSERT ... ON CONFLICT DO UPDATE RETURNING` — Postgres serialises contention with no application-level lock. Pre-dispatch reserves 1 request; if any configured window exceeds its hard limit, dispatch returns `429 Too Many Requests` with a `Retry-After` header and the audit row flips to `failed` with `status_code='denied_quota'`. Post-dispatch, a second update adds `input_tokens` + `output_tokens` to the same buckets. Multiple windows (e.g. 60s / 3600s / 86400s) evaluate in order; first exceeded window wins.

  - **`ai_gateway_policies` table + `PolicyResolver`.** Tenant-scoped JSONB policies composed at dispatch: `allowed_models` (list of model names — anything else returns `403 Forbidden` with audit row `status='failed'`), `max_input_tokens_per_call`, `max_tool_depth`, `quota_windows`, and `safety` (scanner list + block categories). Resolution order: tenant-specific → global (`tenant_id IS NULL`) → compiled-in `GatewayPolicySpec::permissive()` fallback. 60-second in-memory TTL cache; DB unavailability logs a warning and returns the permissive fallback rather than wedging the gateway.

  - **SSE stream tap.** `crates/entry/api/src/services/gateway/stream_tap.rs` wraps the upstream `Stream<Item = Result<Bytes, io::Error>>` and re-emits every chunk to the client byte-identical, while parsing `message_start` / `message_delta` / `content_block_start` / `content_block_delta` / `content_block_stop` frames to accumulate usage + assemble `tool_use` blocks from `input_json_delta` fragments. On end-of-stream, `tokio::spawn` fires `audit.complete(usage, tool_calls, reconstructed_body)`; on upstream error, fires `audit.fail(error)`. The tap never mutates the proxied byte stream — clients that expect byte-exact Anthropic SSE get byte-exact Anthropic SSE.

  - **`x-systemprompt-request-id` response header.** Every gateway response (success, 403 policy denial, 429 quota denial, 451 safety denial, 500 upstream error) carries the minted `AiRequestId` as `x-systemprompt-request-id: <uuid>` so Cowork and any SDK caller can grep logs or the audit table by the same key. Header is also propagated into tracing spans.

  - **Pricing table.** `crates/entry/api/src/services/gateway/pricing.rs` resolves `(provider, model) → ModelPricing { input_cost_per_1k, output_cost_per_1k }` for the Claude 4.x family (Opus / Sonnet / Haiku), MiniMax-* (flat pricing), and GPT-4o family. Unknown pairs log a `WARN` and record `cost_microdollars=0` rather than failing the request, so an operator sees the gap in logs and adds the entry without an incident. Cost computation copies the proven formula from `crates/domain/ai/src/services/core/ai_service/stream_wrapper.rs` (`(input_tokens/1000 × input_cost + output_tokens/1000 × output_cost) × 1_000_000`).

  - **New typed IDs** in `systemprompt_identifiers`: `AiSafetyFindingId`, `AiQuotaBucketId`, `AiGatewayPolicyId` — all generated (UUID-backed) with the `schema` variant for OpenAPI exposure.

  - **New domain repositories** in `systemprompt_ai`: `AiRequestPayloadRepository`, `AiSafetyFindingRepository`, `AiQuotaBucketRepository`, `AiGatewayPolicyRepository`. `AiRequestRepository::insert_with_id(id, record)` is a new public method that lets the gateway audit own ID minting at ingress (the existing `insert(record)` still exists and generates a fresh ID for internal AI-service callers).

  - **`AiRequestRecord.tenant_id: Option<TenantId>`** — new field on the write model + matching `tenant_id()` setter on `AiRequestRecordBuilder`. The underlying `ai_requests` table gained `tenant_id VARCHAR(255)` via migration `001_gateway_governance.sql` with `(tenant_id)` and `(tenant_id, created_at)` indices.

  - **`JwtContextExtractor`-driven user attribution.** The gateway handler extracts `UserId`, `SessionId`, and `TraceId` from the validated JWT context (JWT path) or from the matched `ApiKeyRecord` (API key path), and reads optional `x-tenant-id` from request headers. An `AuthedPrincipal` struct bundles these four fields into a single `GatewayRequestContext` that every downstream module (audit, quota, policy, safety) reads. Previously `JwtContextExtractor::extract_for_gateway` validated the token but its result was discarded.

  - **New dependency edge**: `systemprompt-api` now depends on `systemprompt-ai` for repository access. The gateway service module gained seven new files (`audit.rs`, `parse.rs`, `pricing.rs`, `policy.rs`, `quota.rs`, `stream_tap.rs`, `safety/{mod,heuristic,null}.rs`) and `upstream.rs` was refactored to return a typed `UpstreamOutcome` enum (`Buffered { status, content_type, body } | Streaming { status, stream }`) instead of a raw `Response<Body>`, so the service layer can intercept for audit + policy enforcement before final response assembly.

### Changed

- **Gateway dispatch rewritten around the registry.** `GatewayService::dispatch` is now a thin shim: resolve route → resolve API key → look up the registered upstream → hand off to `upstream.proxy(ctx)`. The old hard-coded `match route.provider { ... }` is gone. The `GatewayProvider` enum (and its `is_openai_compatible()` / `as_str()` methods) have been removed; `GatewayRoute.provider` is a `String`. Anthropic-passthrough and OpenAI-compatible behaviours are preserved — their bodies were moved verbatim into `AnthropicCompatibleUpstream` and `OpenAiCompatibleUpstream` in the new `upstream.rs`. Unknown provider tags now fail fast with `Gateway provider 'xxx' is not registered`.

- **Analytics: broader conversion events + UTM expansion.** `event_data` column on `analytics_events` changed to `JSONB` (was `TEXT`) to support structured payload inspection. Added `utm_content` and `utm_term` UTM parameter columns to complete the full UTM dimension set. Conversion event definitions broadened to cover a wider range of funnel actions (subscription starts, trial activations, feature adoptions).

### Included from 0.2.5

- Workspace-wide Rust-standards sweep (see [0.2.5] entry below for full detail): zero inline comments, zero `unwrap_or_default()`, annotated `serde_json::Value` protocol boundaries, regenerated SQLx offline cache.

---

## [0.2.5] - 2026-04-20

### Changed
- **Workspace-wide Rust-standards sweep.** Executed a full audit against `instructions/prompt/rust.md` and the `rust-coding-standards` skill across `crates/{shared,infra,domain,app,entry}/**/src/`. Five parallel layer agents fixed every zero-tolerance violation they found; a final pass closed the clippy-exposed stragglers. `cargo clippy --workspace --all-targets -- -D warnings` now passes clean, `cargo fmt --all -- --check` is clean, `cargo build --workspace` succeeds. Changes:
  - **Deleted** `crates/shared/models/src/validation_report.rs` — dead 9-line backward-compat re-export, not declared in `lib.rs`, zero importers (all call sites already used `systemprompt_traits::validation_report` directly).
  - **Replaced every `unwrap_or_default()` in src code** (13 occurrences across 7 files). Fixes range from propagating a `Result` (`MarkdownResponse::to_markdown()` now returns `Result<String, serde_yaml::Error>`; its `IntoResponse` impl logs + returns 500 on failure) to idiomatic combinators (`map_or_else(Vec::new, Clone::clone)` in oauth/agent repositories) to explicit `if let Ok(...)` env-var inheritance in agent subprocess spawn. The schema sanitizer's `.next().unwrap_or_default()` became a proper `if let Some(Value::Object(inner))` after an invariant check.
  - **Deleted 19 inline `//` comments** across infra/cloud (4), domain/{ai,agent,analytics,oauth} (14), and entry/cli (15). Per rust.md §3, code documents itself through naming; the only retained `//` annotations are the `// JSON: …` markers on `serde_json::Value` protocol-boundary sites (explicit exception per the `rust-coding-standards` skill).
  - **Annotated ~82 `serde_json::Value` sites in infra** as protocol/schemaless boundaries (A2A JSON-RPC, MCP schemas, webhook payloads, dynamic DB admin queries, log visitors, JSON-Schema trees). Triage reports for all five layers written to `reports/audit/{shared,infra,domain,app,entry}-json-triage.md` (gitignored) with counts of Keep+annotate / Refactor / Defer categories; ~24 refactorable sites and ~106 deferred (API-surface) sites enumerated there for follow-up PRs.
- **Regenerated workspace `.sqlx` offline cache.** Commit `a55b1570e` (analytics conversion + utm) added `utm_content`, `utm_term`, and `event_data` columns to the live DB but the workspace-level sqlx query cache was not regenerated, so `cargo check -p systemprompt-analytics` failed with `SQLX_OFFLINE=true`. Cache now reflects current schema; analytics crate compiles clean again.

### Fixed
- `MarkdownResponse::to_markdown()` signature changed from `fn(&self) -> String` to `fn(&self) -> Result<String, serde_yaml::Error>`. The previous version silently swallowed frontmatter serialization failures via `unwrap_or_default()` and produced a response with no frontmatter. Callers now see the error or (at the HTTP boundary) a logged 500. Breaking for any external consumer of `MarkdownResponse::to_markdown()`; there are none in this repository.

### Audit
- Post-sweep verification greps confirm **zero** occurrences of `.unwrap()`, `unwrap_or_default()`, `panic!`, `todo!`, `unimplemented!`, `unsafe`, `///` doc comments, and `TODO|FIXME|HACK` in any non-test `src/` file across the workspace. `println!`/`eprintln!` retained only at legitimate CLI-output boundaries and in the `config/schema_validation` build-script helper (already guarded with `#[allow(clippy::print_stderr, clippy::print_stdout)]`).

## [0.2.4] - 2026-04-20

### Fixed
- **`admin agents registry` now defaults to the active profile's `api_external_url`.** Previously the command hard-coded `http://localhost:8080` as its gateway URL, so `systemprompt admin agents registry` failed with `Connection refused` on any profile that used a non-default port (e.g. `just setup-local ... 8081 5434`). The hint string on `--url` still advertised "default: http://localhost:8080" even after a user pointed a profile at a different host. Fix: read the active `ProfileBootstrap::get().server.api_external_url` first; fall back to `http://localhost:8080` only if no profile is loaded. `--url` still overrides both.

## [0.2.3] - 2026-04-20

### Fixed
- **Drop cloud-auth requirement for local-trial CLI sessions.** On a fresh template clone with `just setup-local`, the CLI gated a wide set of local-capable operations (`admin agents tools`, `plugins mcp tools/call`, `core contexts list`, trace lookups) behind `Cloud authentication required. Run 'systemprompt cloud auth login' to authenticate.`. Root cause: `SessionKey::from_tenant_id(Some("local_dev"))` returns `SessionKey::Tenant(...)`, not `SessionKey::Local`, so the `session_key.is_local()` branch in `create_new_session` was skipped and `CredentialsBootstrap::require()` fired. `resolve_local_user_email` had the same behavior inside the local-session branch when `session_email_hint` was absent. Fix: centralise the "is this a local-trial profile?" rule on `CloudConfig::is_local_trial()` / `Profile::is_local_trial()` (no `cloud` block, `tenant_id` starts with `local_`, or `validation ∈ {Warn, Skip}`); `create_new_session` now also treats local-trial profiles as local; `resolve_local_user_email` falls back to `admin@localhost.dev` — matching the address `demo/00-preflight.sh` uses, so CLI- and demo-created admin sessions share a user row. Genuine cloud entrypoints (`cloud sync`, `cloud tenant select`, `admin session login`, `admin session switch`) are unchanged and still require cloud credentials. `bootstrap.rs`' duplicated 12-line local-profile predicate now delegates to the shared helper.

## [0.2.2] - 2026-04-17

### Fixed
- **macOS build fix — `statvfs` type mismatch in health endpoint.** `get_disk_usage()` in `systemprompt-api` failed to compile on macOS (Darwin) because `nix::sys::statvfs` returns `u32` for `blocks()`, `blocks_available()`, and `blocks_free()` on macOS but `u64` on Linux, while `fragment_size()` returns platform-varying types. The `saturating_mul` calls required matching types. Fix: explicit `u64::from()` casts on all `statvfs` field accesses so the arithmetic is platform-independent.

### Changed
- Docs sweep: refreshed READMEs across all 30 crates to align with the 0.2.x naming and current feature matrix.
- Relocated generator asset/build/markdown/sitemap unit tests out of `crates/app/generator/tests/` into the dedicated test workspace at `crates/tests/unit/app/generator/src/` to match the "test crates live outside the main workspace" rule. Added missing `unit_tests` module to the scheduler test workspace.

## [0.2.1] - 2026-04-16

### Fixed
- **Idempotent agent migrations — fix startup crash on existing databases.** Migrations `003_a2a_v1_task_states.sql` and `004_ai_requests_task_fk.sql` could brick service startup on sites with pre-existing data. Root cause: `SqlExecutor::execute_statements_parsed` splits SQL on semicolons and runs each statement as a separate `execute_raw` call against the connection pool, so the `BEGIN`/`COMMIT` wrapper in migration 003 was a no-op (each statement auto-committed on potentially different connections). If any statement succeeded but the migration recording failed, the next startup retried the migration and hit already-applied DDL. Three fixes: (1) removed the ineffective `BEGIN`/`COMMIT` from migration 003, (2) added missing `UPDATE` for `'pending'` → `'TASK_STATE_PENDING'` status value that would cause the CHECK constraint to reject existing rows, (3) wrapped the `ADD CONSTRAINT` in migration 004 with an `IF NOT EXISTS` guard via a `DO` block so re-running the migration after a partial failure is safe.
- **Gemini schema sanitizer — nullable & $ref handling.** `ProviderCapabilities::gemini()` now reports `features.references = false` and `features.definitions = false`, so the sanitizer strips `$ref` / `$defs` / `definitions` before the request reaches Gemini. Gemini's `FunctionDeclaration.parameters` uses `google.api.JsonSchema`, which rejects those keywords with `400 INVALID_ARGUMENT`.
- **Nullable normalisation in `SchemaSanitizer`.** New `normalize_nullable` pre-pass rewrites both JSON-Schema nullable forms into Gemini/OpenAPI `nullable: true`: `{"type": ["string", "null"]}` collapses to `{"type": "string", "nullable": true}`, and `{"anyOf": [{"type": "X"}, {"type": "null"}]}` collapses to `{"type": "X", "nullable": true}`. Non-null `anyOf` unions and `type` arrays without a `"null"` sibling are left untouched. Runs before composition stripping so the result survives the rest of the pipeline.
- **Analytics — per-agent cost breakdown reconciles with totals.** `CostAnalyticsRepository::get_breakdown_by_agent` now returns an always-present `'unattributed'` aggregate row alongside the top-N attributed agents, via a `UNION ALL` of (INNER JOIN'd attributed spend) + (unattributed spend with `task_id IS NULL OR agent_name IS NULL`). The invariant `sum(breakdown_by_agent.cost) == get_summary().total_cost` now holds for every window. An in-flight edit had switched to a plain `INNER JOIN`, silently dropping ad-hoc / context-less AI spend from the governance audit — exactly the shadow-AI blindspot the report exists to surface. `LIMIT` only bounds the attributed top-N; the unattributed row is never truncated. Four new reconciliation tests in `crates/tests/unit/domain/analytics/src/repository/costs.rs` lock the invariant in place (all-attributed, mixed-null, limit-survival, empty-window).
- **Agent extension — registered unreleased `003_a2a_v1_task_states.sql` migration.** Found during this release: `crates/domain/agent/schema/migrations/003_a2a_v1_task_states.sql` was added during the 0.1.22 A2A v1 protocol upgrade but never registered in `AgentExtension::migrations()`, so the live UPDATE that rewrites legacy `submitted`/`working`/... rows to `TASK_STATE_*` SCREAMING_SNAKE_CASE and tightens the CHECK constraint had never run on any deployed instance. Any database with pre-0.1.22 task rows would have been in an inconsistent state. Migration is now wired up and runs on next migration sweep.

### Schema
- **`ai_requests.task_id` is now a proper FK to `agent_tasks(task_id)`.** New migration `crates/domain/agent/schema/migrations/004_ai_requests_task_fk.sql` normalises the column type from `VARCHAR(255)` to `TEXT` (matches parent PK), nulls out pre-existing orphaned references (preserving cost/token data), and installs `FOREIGN KEY (task_id) REFERENCES agent_tasks(task_id) ON DELETE SET NULL`. From here on, orphaned `task_id` values are structurally impossible, and deleting an agent task rolls its historical AI spend up under `'unattributed'` in the cost breakdown rather than cascading away audit data. `systemprompt-agent` now declares `"ai"` as an explicit extension dependency so the migration runs after the `ai_requests` table exists. Migration placement rationale: ai (weight 35) loads before agent (40), so a cross-domain FK from `ai_requests → agent_tasks` must be installed from the agent side.

### Removed — Dead `CreateAiRequest` insert path
- Deleted `CreateAiRequest` struct and `AiRequestRepository::create()` method from `crates/domain/ai/src/repository/ai_requests/`, plus associated re-exports in `crates/domain/ai/src/lib.rs`, `repository/mod.rs`, and `ai_requests/mod.rs`. The struct had no `task_id` field and no production callers; its existence invited a future bug where a new caller would use it and produce unattributable AI spend rows. The live insert path remains `AiRequestRecord` + `AiRequestRepository::insert()`, which already carries `task_id: Option<TaskId>`. BREAKING for any external crate importing `CreateAiRequest`; there are none in this repository.

### Chores
- Workspace bumped to 0.2.1; per-crate descriptions swept (b5b13d59c).
- **Cargo feature-flag sweep.** Removed unused / always-on feature gates across the workspace: `systemprompt-extension` (`web`, `plugin-discovery`), `systemprompt-logging` (empty `web`), `systemprompt-database` (`api` + dead optional `axum`), `systemprompt-mcp` (empty `cli`), `systemprompt-oauth` (`web`), `systemprompt-agent` (`web`, empty `cli`), `systemprompt-analytics` (`web`), `systemprompt-scheduler` (empty block), `systemprompt-cloud` (empty `test-utils`). Inlined previously-optional deps (axum, tower, tower-http, bytes, jsonwebtoken, tokio-stream, urlencoding) and stripped ~40 `#[cfg(feature = ...)]` gates. Legitimate gates kept: `models/web`, `traits/web`, `identifiers/sqlx`, `template-provider/tokio`, `logging/cli`, `runtime/geolocation`, `analytics/geolocation`, `generator/image-processing`, and the facade crate's user-facing feature matrix.

### Services Config Migration (Phases 1-4)

A workspace-wide breaking change to the services configuration layer.

- **Phase 1 — Schema**: `ServicesConfig` grew first-class `skills` and `content` fields; `PluginConfig` gained `content_sources` bindings; both `ServicesConfig` and `PartialServicesConfig` are locked with `#[serde(deny_unknown_fields)]`; `ServicesConfig::validate()` now enforces plugin bindings and skill map-key integrity.
- **Phase 2 — WebConfig**: deleted the 3-field stub `WebConfig` in `systemprompt-models` and switched `ServicesConfig.web` to `Option<systemprompt_provider_contracts::WebConfig>` so the rich branding/colors/typography/layout config round-trips through the loader. Breaking for any caller constructing the stub directly.
- **Phase 3 — Loader**: `ConfigLoader` is now the single loader with recursive `includes:` resolution and cycle detection. Removed `EnhancedConfigLoader`, `IncludeResolver`, `ConfigLoader::discover_and_load_agents`, and `ConfigWriter::add_include`. Loading is now pure — no auto-discovery side effects on `config.yaml`. Users must list every include explicitly.
- **Phase 4 — Callers**: `cloud profile show` and all remaining call sites migrated to `ConfigLoader::load()`.

### Phase 5 — Typed-ID migration (trait surfaces + DTOs)

- Migrated `ContextProvider`, `UserProvider`, `RoleProvider` trait surfaces from raw `&str` to typed identifiers (`UserId`, `ContextId`, `SessionId`). Breaking for any external impl.
- Waves 1–5 (commits 13568bcfa…806cc2844) covered canonical models, A2A protocol, oauth/webauthn, AI rows, tracing, app sync/generator, and CLI residuals.
- DTO sweep: migrated the remaining raw `String` ID fields across cloud DTOs, services models, AI rows, analytics events, A2A protocol messages, and API/CLI surfaces to `systemprompt_identifiers` typed IDs. Serialization is unchanged (typed IDs round-trip as plain strings).
- Wave 7 — **completed**: all 69 remaining raw `String` ID fields across shared traits, shared models, infra (security claims), domain (users, analytics, ai, oauth, agent), app/sync, entry/api webauthn+anonymous+proxy, and entry/cli plugins/content/logs migrated to typed identifiers. `LogId` gained `JsonSchema` support. `WebAuthnService::finish_registration_with_token` and `WebAuthnService::finish_registration` now return `UserId` instead of `String`. Vendor/external IDs (WebAuthn FIDO2 credentials, A2A third-party agent-card skill IDs, third-party webhook endpoint IDs, external LLM model names, CTA button action identifiers) kept as `String` with `// JSON:` justification comments per the narrow exception in CLAUDE.md. Clap CLI arguments that accept user-provided partial lookups (`ShowArgs.id`, `AuditArgs.id`, etc.) annotated with `// CLI:` and kept as `String` by design.

### Removed — Dead authorization stubs

- Deleted `crates/domain/oauth/src/services/auth_provider.rs` in its entirety. `JwtAuthProvider`, `JwtAuthorizationProvider`, and `TraitBasedAuthService` were dead since v0.0.1: zero production callers, and `JwtAuthorizationProvider::{authorize, get_permissions}` silently returned `Ok(true)` / `Ok(vec![])` regardless of input — a latent authorization footgun. Real permission logic continues to live in `JwtClaims::get_permissions()` and `crates/domain/mcp/src/middleware/rbac.rs`.
- Collapsed the `AuthorizationProvider` trait and `AuthProvider` trait entirely — both were single-impl traits with no call sites. Removed associated dead types: `AuthAction`, `AuthPermission`, `TokenPair`, `TokenClaims`, `DynAuthProvider`, `DynAuthorizationProvider`. BREAKING for any external crate importing these names; there are none in this repository.
- Removed `JwtAuthProvider::{refresh_token, revoke_token}` which returned `"not yet implemented"` errors and had zero callers. The real OAuth refresh/revoke lifecycle uses `OAuthRepository` and the token endpoints — unaffected.

### Fixed

- Zero-warning, zero-error build across workspace (`cargo build --workspace` + `cargo clippy --workspace --all-targets -- -D warnings` + `cargo fmt --all --check`).
- Resolved clippy `needless_borrow` in `crates/entry/api/src/routes/oauth/endpoints/anonymous.rs` and `.../token/generation.rs`.
- Resolved clippy `useless_conversion` and `single_match_else` in `crates/entry/cli/src/commands/admin/agents/message.rs` and `.../cloud/sync/admin_user/sync.rs`.
- Dropped unused parameters in `AgentOrchestrationDatabase::{mark_failed, get_unresponsive_agents}`, `MonitorService::cleanup_unresponsive_agents`, and `a2a_server::handlers::request::validation::should_require_oauth` — signatures no longer lie about what the implementation uses.
- Removed 15 forbidden doc comments from `crates/shared/models/src/macros.rs` (standards: no `///` in production code).
- Removed 1 unnecessary path qualification in `crates/domain/agent/src/services/a2a_server/auth/validation.rs`.

## [0.1.22] - 2026-04-07

### Changed
- **A2A Protocol v1.0.0 Migration** — upgrade from v0.3.0 to the first stable release (Linux Foundation, March 2026)
  - TaskState: kebab-case to `TASK_STATE_*` SCREAMING_SNAKE_CASE (`"submitted"` -> `"TASK_STATE_SUBMITTED"`)
  - MessageRole: `"user"`/`"agent"` to `"ROLE_USER"`/`"ROLE_AGENT"`, now a typed enum
  - Part: tagged enum (`kind` discriminator) to untagged (field-presence discrimination)
  - FileWithBytes renamed to FileContent; `bytes` now optional, added `url` field for URL-referenced files
  - Message: removed `kind` field, `id` renamed to `message_id`
  - Task: removed `kind` field, added `created_at`/`last_modified` timestamps
  - Artifact: `name` renamed to `title`
  - AgentCard: collapsed `url`/`preferred_transport`/`additional_interfaces` into `supported_interfaces` array with per-interface protocol version
  - TransportProtocol renamed to ProtocolBinding (type alias kept)
  - JSON-RPC methods: PascalCase (`"message/send"` -> `"SendMessage"`, `"tasks/get"` -> `"GetTask"`, etc.)

### Fixed
- Resolve all build warnings and clippy errors across workspace
  - Add missing `Debug` derives on `BuildMetadataParams`, `HtmlBuilder`, `TokenGenerationParams`, `AuthCodeValidationParams`
  - Fix ambiguous glob re-export of `validation` module in OAuth endpoints
  - Allow `struct_field_names` on A2A `Message` (protocol-required field name)
  - Replace redundant closures with function references in agent URL extraction
  - Add `const fn` to `TaskState::is_terminal()`, `can_transition_to()`, and `role_to_str()`
  - Use `Self` instead of concrete type in `TaskState::can_transition_to()` parameter

### Added
- Database migration `003_a2a_v1_task_states.sql` for task status value migration
- TaskState `is_terminal()` and `can_transition_to()` methods for state machine validation
- Backward-compatible task state parsing (accepts both old and new format strings)

## [0.1.21] - 2026-04-01

### Fixed
- Remove silent error swallowing in `DatabaseLayer::flush()` — all DB log write failures are now reported with entry count
- Logging initialization order: `init_logging(db_pool)` now works regardless of whether `init_console_logging()` was called first

### Changed
- Replace `DatabaseLayer` with `ProxyDatabaseLayer` architecture — subscriber is always initialized with a proxy that accepts a DB pool attachment at any time
- Move `AppContext` construction logic from `new_internal()` into `AppContextBuilder::build()` — builder owns its construction
- Move `init_logging()` call earlier in `AppContextBuilder::build()` — immediately after DB pool creation, before extension discovery
- Extract `AppContextBuilder` into `crates/app/runtime/src/builder.rs`
- Extract `ProxyDatabaseLayer` and shared span/event helpers into `crates/infra/logging/src/layer/proxy.rs`
- Remove redundant `init_logging()` call from `serve.rs`

## [0.1.20] - 2026-04-01

### Changed
- Upgrade `rmcp`/`rmcp-macros` from 1.1 to 1.3
- Simplify MCP `StreamableHttpServerConfig` to use library defaults instead of manual field construction
- Adapt MCP HTTP client to rmcp 1.3 API: replace removed `AuthRequiredError` with `UnexpectedServerResponse`
- Rebrand README messaging: reposition from "production infrastructure for AI agents" to "AI governance layer" with compliance-first positioning (SOC 2, ISO 27001, HIPAA, FedRAMP)
- Update README navigation: "Playbooks" → "Skills"

### Added
- `ensure_project_scaffolding()` function in cloud init — auto-creates `services/` and `web/` directories during local tenant setup
- Project scaffolding step integrated into local tenant creation workflow (runs before profile setup)

### Refactored
- Resolve all remaining clippy errors and warnings to achieve zero-warning build
- Introduce parameter structs for `too_many_arguments` in agent services (Wave 2)
- Eliminate all redundant closure violations (Wave 1)
- Split large files: complete `deploy/mod.rs` split and file split extractions from source files
- Remove `unsafe` blocks and convert static SQL to compile-time verified macros

### Removed
- Clean up ~120 stale SQLx query cache files from sync crate

## [0.1.19] - 2026-03-31

### Added
- `CloudEnterpriseLicenseInfo` struct for domain-based enterprise licensing
- `enterprise` field on `UserMeResponse` (optional, backward-compatible)
- `EnterpriseLicenseInfo` type alias
- Structured streaming with `StreamChunk` enum for typed AI provider responses with token usage tracking
- Pricing-based cost calculation for streaming responses
- Authenticated `/api/v1/health/detail` endpoint with full system diagnostics (split from public health check)
- Email validation module (`validation.rs`) with shared `is_valid_email` helper
- ConnectInfo fallback for IP extraction in bot detector and IP ban middleware
- `geolocation` feature flag for optional GeoIP/MaxMind dependency in analytics and runtime

### Changed
- Simplify public `/health` endpoint to a lightweight DB-only check (fast for load balancers)
- Replace `tokio::process::Command("df")` disk usage with synchronous `libc::statvfs` syscall
- Make `CliService` conditionally compiled behind `cli` feature flag in logging crate
- Reduce default tokio features in workspace (remove `fs`, `process`, `signal` from default set)
- Replace blocking `std::sync::Mutex` with `tokio::sync::Mutex` in Gemini AI provider to prevent tokio worker thread stalls
- Agent sub-processes now start with a clean environment (`env_clear`) instead of inheriting all parent secrets
- Filter system traces and unknown status from trace list by default

### Security
- Fix OAuth redirect URI bypass: full URLs can no longer match relative URI registrations
- Fix WebAuthn user ID spoofing: completion handler now verifies authenticated user identity via auth token instead of trusting query parameter
- Remove wildcard CORS headers from WebAuthn completion endpoint
- Enforce 120-second expiry on WebAuthn registration and authentication challenges
- Add Shannon entropy validation for PKCE code challenges
- Block internal/private IP addresses in OAuth resource URI validation
- Use constant-time comparison (`subtle` crate) for sync token authentication
- Block symlinks and hardlinks in tarball extraction with canonical path validation
- Unify authorization code error messages to prevent enumeration attacks

### Refactored
- **CLI architecture remediation**: eliminate all `unwrap_or_default()`, `unsafe`, unlogged `.ok()`, and `println!()` violations across 8 CLI domains (admin, analytics, cloud, core, infrastructure, plugins, web, build)
- Split 14 oversized CLI files (>300 lines) into focused submodules — zero files now exceed the 300-line limit
- Extract magic numbers to named constants across analytics and infrastructure commands
- Refactor long functions (>75 lines) in analytics agents/show, sessions/live, and tools/show
- Replace `unsafe { std::env::set_var() }` in cloud profile/sync with safe `ProfileBootstrap::init_from_path()` config propagation
- Replace raw `std::env::var()` calls in cloud commands with Config-based alternatives
- **Struct consolidation**: rename duplicate `ToolModelConfig` (all-optional) to `ToolModelOverride`, resolve `Settings` collision into `ServicesSettings`/`DeploymentSettings`, deduplicate `RenderingHints` (CLI now imports from models crate)
- Convert `ToolContext` ID fields from raw `String` to typed identifiers (`SessionId`, `TraceId`, `AiToolCallId`)
- Convert image generation model ID fields from raw `String` to typed identifiers (`UserId`, `SessionId`, `TraceId`, `McpExecutionId`)
- **Eliminate inline SQL from CLI**: move 10 inline queries from `logs/show.rs`, `logs/export.rs`, and `logs/summary.rs` to `TraceQueryService` with dedicated query modules (`log_lookup_queries.rs`, `log_summary_queries.rs`)
- **Typed IDs for trace models**: replace 6 remaining `String` ID fields with typed identifiers (`LogId`, `AiRequestId`, `ExecutionStepId`) across `LogSearchItem`, `AiRequestListItem`, `AiRequestDetail`, `AuditLookupResult`, `ExecutionStep`, `AiRequestInfo`
- **DRY identifier definitions**: consolidate hand-written identifier structs into `define_id!()` macro invocations, removing ~2,500 lines of duplicated boilerplate across 14 identifier modules
- Consolidate shared utilities and per-crate `.sqlx/` caches for publish workflow
- Config cleanup: encapsulate visibility, remove dead code across config and logging crates
- **Code quality sweep across all layers** (139 files): remove clippy suppressions, fix forbidden constructs, eliminate silent error patterns
  - Remove `#[allow(clippy::*)]` suppressions by fixing underlying issues: `cognitive_complexity` (split functions), `too_many_arguments` (parameter structs), `struct_excessive_bools` (bitflags/enums), `print_stdout` (CliService::output/std::io::Write), `expect_used` (proper error propagation), `unnecessary_wraps`, `struct_field_names`, `empty_structs_with_brackets`, `option_option` (CategoryIdUpdate enum), `enum_variant_names`
  - Replace `CommandDescriptor` 6-bool struct with u8 bitflags pattern and const accessor methods
  - Introduce parameter structs: `TenantSessionParams`, `NonStreamingRequest`, `SessionStoreParams`, `ToolCallParams`, `TrackingParams`, `ReconcileSuccessParams`, `BuildContextParams`, `AuthCodeValidationParams`
  - Remove anyhow bridges in `AiError` and `AgentError`: replace `DatabaseError(#[from] anyhow::Error)` with `DatabaseError(String)`
  - Replace `println!`/`eprintln!` with `std::io::Write` across infra/logging CLI display and startup validation
  - Fix all `unwrap_or_default()` in CLI and domain code with explicit error handling
  - Fix silent error patterns: convert `let _ =` and `.ok()` to `tracing::warn!` or proper propagation across agent, mcp, scheduler, runtime, and API layers
  - Replace `Vec<EndpointRateLimit>` for rate limit config (eliminate struct_field_names)
  - Split `ProviderCapabilities` into `SchemaComposition` + `SchemaFeatures` sub-structs
  - Replace `process::exit()` with proper error propagation in CLI bootstrap
- Extract trace/logging queries into dedicated modules (`log_search_queries.rs`, `request_queries.rs`, `tool_queries.rs`, etc.)
- Remove dead `show_helpers.rs` and unused agent lib.rs clippy allow-list
- **Module visibility hardening**: convert `pub mod` to `pub(crate) mod` for internal modules across 7 domain crates (agent, ai, analytics, users, oauth, content, mcp) — reduces public API surface while preserving re-exports
- **Rename `models::ContentError` to `ContentValidationError`**: resolve naming collision with the operational `error::ContentError` in the content crate
- Fix `McpCspDomains` field references (`connect_domains` -> `connect`, `resource_domains` -> `resources`, etc.)
- Fix `BuildContextParams` call sites to use struct construction instead of positional args
- **Coding standards compliance sweep**:
  - Delete 20 dead `.rs` files and 4 dead `.sql` files not declared in any `mod.rs`
  - Convert 7 static `sqlx::query()` calls to compile-time verified `sqlx::query!()` / `sqlx::query_scalar!()` macros
  - Remove `unsafe` block in config manager: replace `std::env::set_var` with in-process `HashMap` for secret resolution
  - Remove `unsafe` block in health check: replace `libc::statvfs` FFI with `nix::sys::statvfs` safe wrapper
  - Split 6 files exceeding 400 lines into focused submodules: `audit_queries.rs`, `ai_trace_display.rs`, `secrets_bootstrap.rs`, `file_bundler.rs`, `deploy_steps.rs`, `profile_steps.rs`
- Fix `Arc<AnalyticsService>` to `Arc<dyn AnalyticsProvider>` coercion in session middleware
- Fix `CloudPaths` API consumers after `get_cloud_paths()` return type change
- Remove unused `_pool` parameter from `CleanupRepository::new_with_write_pool`

### Fixed
- Sub-process binary resolution now checks both `target/release` and `target/debug`, preferring the newest by mtime — matches justfile behavior so MCP servers and agents find the correct binary during development
- MCP binary validation uses dynamic bin directory resolution instead of hardcoding `target/release`
- Fix test compilation across `systemprompt-generator` and `systemprompt-sync`
- Remove needless `..Default::default()` in API JWT config
- Fix `bool as Option<bool>` invalid cast in trace list queries
- Populate AI trace summary fields (`total_cost`, `total_tokens`, `total_latency`) that were previously always zero

## [0.1.18] - 2026-03-05

### Changed
- Upgrade Rust edition from 2021 to 2024
- Reorder imports across all crates to comply with Rust 2024 edition formatting rules
- Change `unsafe_code` workspace lint from `forbid` to `deny`
- Parallelize prerender pipeline: concurrent source processing, item rendering, and content enrichment
- Replace regex-based TOC heading ID injection with string search (removes `regex` dependency from generator)

### Removed
- Remove TUI OAuth client seed data and configuration
- Remove TUI testing plan
