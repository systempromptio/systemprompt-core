# Changelog

## [0.15.3] - 2026-06-10

### Breaking

- Error enum tuple variants that wrapped a bare message string are now struct variants with a named `message` field; match arms and constructors change from `Error::Foo(msg)` to `Error::Foo { message: msg }`.
- The minimum supported Rust version is 1.88.

### Changed

- Over-long functions were split into focused helpers to satisfy the workspace's 75-line function ceiling. No behavioural or API change.

### Fixed

- Cloud-sync tar and gzip transfers run on blocking threads instead of stalling the async runtime.

## [0.14.0] - 2026-06-01

### Changed

- Gateway outbound dispatch resolves providers from the `systemprompt-models` provider registry and uses the relocated `wire::*` codecs. A Gemini outbound adapter is added, and the duplicated per-protocol request/response/streaming modules under the gateway are removed in favour of the shared codecs. The gateway threads the expanded canonical fields — grounding and citation evidence, code-execution output, cache and total token usage, image `detail`, and sampling penalties — through dispatch in both directions.

## [0.13.1] - 2026-06-01

### Changed

- The plugin manifest and plugin-file routes build from the shared `systemprompt-marketplace` bundle source, so served bytes and manifest hashes share one definition. Gateway dispatch resolves routes through `GatewayConfig::resolve_route`, forwarding a model unmatched by any explicit route to the configured `default_provider` instead of denying it.

### Removed

- The unused `openai_chat_completions::render` module.

## [0.13.0] - 2026-05-28

### Changed

- `routes::oauth::endpoints::register::register_client` applies RFC 7591 §2 defaults when the dynamic-client-registration request omits `grant_types` or `response_types`: missing or empty arrays resolve to `["authorization_code"]` and `["code"]` respectively. The same defaulted values flow into the repository insert and the response body, keeping the persisted client and the registration echo in sync. Spec-compliant MCP clients (Cowork, Claude Code DCR, MCP Inspector) no longer hit `400 invalid_client_metadata` on minimal registration payloads.
- `routes::gateway::bridge_data::load_managed_mcp_servers` synthesises the public MCP URL from `api_external_url + /api/v1/mcp/<name>/mcp` whenever the deployment's `endpoint` is absent or relative. Absolute URLs are only honoured for `external` servers; absolute endpoints on `internal` servers are rejected at config-load time.
- `services::proxy::auth::OAuthChallengeBuilder` distinguishes the no-credentials case from the bad-credentials case on `/api/v1/mcp/*` 401 responses. When no `Authorization` header is present, the `WWW-Authenticate: Bearer` challenge omits `error=` per RFC 6750 §3 — the spec-compliant signal that clients should begin the OAuth flow rather than treat the response as a token rejection. When a malformed or invalid token is present, the previous `error="invalid_token"` form is retained.
- `services::proxy::auth::OAuthChallengeBuilder` derives the `WWW-Authenticate: Bearer resource_metadata="…"` URL from the incoming request's `Host` header through the same `RequestBaseUrl` resolver the `.well-known/oauth-protected-resource` body uses, closing the host-of-truth gap that left the discovery body and the 401 challenge advertising different hosts on RFC 9728 dual-self-identity gateways. Host-header injection is bounded by the configured-host allowlist (with loopback aliases when applicable); non-allowlisted hosts fall back to `api_external_url`.
- Route-mount context middleware is now four typed sibling layers — `PublicContextMiddleware`, `UserOnlyContextMiddleware`, `A2AContextMiddleware`, `McpContextMiddleware` — each implementing the new sealed `ContextLayer` trait that `RouterExt::with_auth` accepts. Each flavour's contract (Anon admission, optional-header merge, body-rebuild, MCP session-context fallback) is expressed at the type level rather than via a runtime `ContextRequirement` enum branch.
- `extraction_error_to_api_error` is now a module-level free function in `services::middleware::context::middleware`. It does not depend on the middleware instance.
- `client_credentials` no longer intersects service-tier scopes (`hook:govern`, `hook:track`, `service`, `a2a`, `mcp`) with the OAuth client owner's roles. RFC 6749 §4.4 has no resource owner in the loop; service-tier scopes are statically granted to the client at registration and the `owner_user_id` is retained for audit attribution only. User-tier scopes (`admin`, `user`, `anonymous`) continue to require both the client grant and the owner's roles, matching the on-behalf-of delegation contract. `ClientCredentialsError::InvalidScope` now names the actual deficit — `requested scopes not in client grant: …` or `delegated scopes not held by owner: …` — instead of the generic `scopes not allowed for both client and owner`.

### Removed

- `ContextMiddleware`, its `public` / `user_only` / `full` / `mcp` constructors, and the `ContextRequirement` enum are deleted.
- `ContextExtractor::extract_user_only` is folded into `extract_from_headers`. The single implementor had identical bodies for both.

### Fixed

- `/api/v1/mcp/*` mounts under `AuthzPolicy::public()` so the proxy handler (`services/proxy/auth.rs::AccessValidator`) can answer unauthenticated requests with the RFC 9728 `WWW-Authenticate: Bearer resource_metadata="…"` 401 challenge it already builds. v0.11.0 inserted a redundant `AuthzPolicy::restricted_to([User, Admin, Mcp, Service])` gate above the proxy, which short-circuited the request to a generic 403 (`caller type 'anon' is not authorized for this route`) and prevented spec-compliant MCP clients from starting their OAuth discovery handshake. Regression coverage: unit tests on `AuthzPolicy`/`authz_gate` in `crates/tests/unit/entry/api/src/middleware/authz_policy.rs` and an integration test driving the full mounted router in `crates/tests/integration/api/routes_mcp_unauth_challenge.rs`.
- Unauthenticated or malformed-bearer requests to `/api/v1/mcp/<unknown>/…` now receive the RFC 9728 401 challenge instead of `404 Service not found`. `services::proxy::engine::proxy_request` intercepts `ServiceNotFound` on the MCP branch and promotes it to the existing `OAuthChallengeBuilder` challenge whenever the request was not properly authenticated; authenticated callers continue to receive 404 for a genuinely unknown service. Required so spec-compliant MCP clients can begin OAuth discovery against any `/api/v1/mcp/*` path.

## [0.12.1] - 2026-05-27

### Fixed

- `services::server::metrics::install_recorder` caches the `PrometheusHandle` in a process-wide `OnceLock`. Repeated calls in the same process (multiple `setup_api_server` calls in one test binary, or any future re-bootstrap path) return a clone of the existing handle instead of erroring with "attempted to set a recorder after the metrics system was already initialized".

## [0.12.0] - 2026-05-27

### Breaking

- Gateway authz path forwards `JwtUserContext.attributes` onto every `AuthzRequest` and mints `AuthzContext::gateway_invocation(&ModelId)` at the enforcement site. Routes consuming the old `AuthzRequest.department` field no longer compile.
- Gateway derives `ContextId` from `GatewayConversationId` via UUID v5; upstream `x-context-id` headers on gateway routes are ignored. MCP and A2A surfaces continue to honour `x-context-id`.

### Added

- Bootstrap composes `[RuleBasedHook, ...extensions]` automatically when a DB pool is available so the core RBAC resolver runs as a first-class hook; `mode: webhook` composes `[RuleBasedHook, WebhookHook]`. The implicit "resolver runs before the hook" flow is gone — every decision is a hook now.

## [0.11.0] - 2026-05-20

### Breaking
- Sync routes drop the `SYNC_TOKEN` middleware and gate on `with_auth(_, AuthzPolicy::restricted_to(&[Service]))`. Sync clients must mint a `client_credentials` Service-JWT.

### Added
- `RouterExt::with_auth(_, AuthzPolicy::*)` registration: every authenticated route declares its authz tier at compile time; routes that forget to install a guard fail to compile.
- `services/middleware/served_by.rs` middleware tagging each response with the serving replica identity (for load-balancer fairness measurement and Prometheus labelling).
- Prometheus metrics endpoint.

### Changed
- Every per-item `///` rustdoc in `entry/api` is removed in line with the standing rustdoc rule; file-level `//!` blocks describe purpose where the value is real.
- Gateway route extraction reflects the tenancy strip in `domain/ai` and `domain/oauth`: handlers no longer extract or thread a runtime `tenant_id`.

## [0.9.2] - 2026-05-14

### Changed
- Normalized changelog formatting for consistency with downstream crate conventions.

## [0.3.0] - 2026-04-22

### Changed
- Gateway quota update API now takes a `PostUpdateParams` struct.
- Gateway request finalization moves owned values into spawn tasks instead of cloning.
- `manifest_signing::signing_key` handles concurrent initialization without panicking.

## [0.2.2] - 2026-04-17

### Fixed
- Disk usage probe in the health endpoint builds on macOS where `statvfs` field widths differ from Linux.

## [0.2.0] - 2026-04-15

### Fixed
- Removed redundant borrow in the anonymous OAuth admin JWT issuer.
- Removed redundant borrow when recording OAuth client last-used timestamps.

## [0.1.21] - 2026-04-02

### Changed
- Sync, analytics, and admin routes now resolve their paths through `ApiPaths` constants instead of hard-coded strings.
- MCP registry endpoint URLs now resolve through `ApiPaths::mcp_server_endpoint()`.

## [0.1.17] - 2026-03-20

### Fixed
- Removed redundant `..Default::default()` spread in JWT config construction.

## [0.1.16] - 2026-03-05

### Changed
- Dropped `form_post` from the supported response modes advertised by OAuth discovery metadata.
- Simplified scope resolution in the OAuth authorize endpoint.
- Removed redundant resource-scope validation from the token endpoint.
- Removed unused `McpServerRegistry` and `McpRegistryProvider` imports from authorize validation.

## [0.1.15] - 2026-02-19

### Changed
- `site_auth_gate` now requires an exact permission match instead of hierarchy-based `implies()`.

## [0.1.14] - 2026-02-18

### Changed
- `site_auth_gate` is now expressed as an iterator chain.
- Token extraction and JWT validation failures in site auth now emit structured `tracing` events.

## [0.1.13] - 2026-02-11

### Changed
- OAuth authorize template now receives `register_class` derived from `Config.allow_registration`.

## [0.1.12] - 2026-02-11

### Added
- Security headers middleware (`inject_security_headers`) covering HSTS, X-Frame-Options, X-Content-Type-Options, Referrer-Policy, Permissions-Policy, and CSP.
- Health endpoint exposes database size, top tables, disk usage, and audit log metrics.
- Path-based `/.well-known/oauth-protected-resource/{*path}` endpoint for per-MCP-server resource metadata.
- `refresh_token` grant type in MCP authorization server metadata.
- ETag and `If-None-Match` support on static file responses, returning `304 Not Modified` on match.
- `Cache-Control: no-cache` on HTML responses and `Cache-Control: public, max-age=3600` on metadata files (sitemap, robots, feed).

### Changed
- MCP OAuth metadata is emitted using typed enums (`ResponseType`, `GrantType`, `PkceMethod`, `TokenAuthMethod`).
- `resource_documentation` in protected-resource responses now uses the base URL.
- Static file handlers now use `tokio::fs::read()` instead of blocking I/O.
- Renamed the static file module from `vite.rs` to `static_files.rs`.

### Fixed
- Restored the Claude Code OAuth flow by removing the `Accept` header check that blocked programmatic clients.

### Removed
- Dead `serve_html_with_analytics` helper (analytics are handled by middleware and client JS).

## [0.1.11] - 2026-02-08

### Added
- Content routing in analytics and engagement routes, resolving content IDs from URL slugs.
- `AnalyticsState` and `EngagementState` now carry content routing.

### Fixed
- `record_events_batch` now forwards content routing to `resolve_content_id`.

## [0.1.10] - 2026-02-06

### Added
- Site-wide authentication gate middleware (`site_auth_gate`).
- Extensions can declare site auth requirements via a new `site_auth()` trait method.
- Unauthenticated static content requests now redirect to the configured login path.
- Static assets and extension-declared public prefixes bypass the auth gate.

## [0.1.9] - 2026-02-05

### Added
- Content negotiation middleware with an `AcceptedFormat` extractor and `AcceptedMediaType` enum supporting JSON and Markdown.
- `.md` URL suffix is recognized as a Markdown format request.
- Content responses now include an HTTP `Link` header pointing to alternate formats.

### Changed
- Content handlers now receive `AppContext` instead of `DbPool`.
- Blog content endpoint returns Markdown when requested via `Accept: text/markdown` or a `.md` suffix.

## [0.1.4] - 2026-02-04

### Added
- RFC 8707 `resource` parameter support on the authorize and token endpoints, with HTTP(S) URI validation.
- `TokenGenerationParams.resource` field for resource-scoped tokens.

### Changed
- `AuthorizeQuery`, `AuthorizeRequest`, `TokenRequest`, and `WebAuthnCompleteQuery` now carry a `resource` field.
- WebAuthn form template context now includes `resource`.

## [0.1.3] - 2026-02-03

### Removed
- Webhook publisher configuration from `create_oauth_state()`; cloud activity API is used instead.

## [0.1.2] - 2026-02-03

### Changed
- Regenerated the SQLx offline query cache.

## [0.1.1] - 2026-02-03

### Fixed
- Session middleware now creates a fresh anonymous session when a JWT references a missing user instead of returning an error.

## [0.1.0] - 2026-02-02

### Changed
- First stable release; all workspace crates aligned at 0.1.0.

## [0.0.13] - 2026-01-27

### Changed
- Artifact response builder now uses `expect()` with a message instead of `unwrap()`.

## [0.0.11] - 2026-01-26

### Added
- Engagement metrics fan out from `PageExit` events in analytics routes.
- Batched analytics event processing with engagement fan-out.
- `JwtContextExtractor` validates and auto-creates sessions for OAuth tokens issued before the session persistence fix.

### Changed
- Renamed `AnalyticsState` fields to drop the redundant `_repo` postfix.
- Improved session middleware handling.

## [0.0.3] - 2026-01-22

### Added
- Migration system infrastructure.

### Fixed
- Schema validation now accepts view-based schemas.

## [0.0.2] - 2026-01-22

### Changed
- Each domain crate now owns its SQL schemas through the `Extension` trait; centralized loaders in `systemprompt-loader` are gone.

### Fixed
- `include_str!` paths now resolve inside the crate so it compiles standalone from crates.io.

## [0.0.1] - 2026-01-21

### Added
- Initial release.
