# Changelog

## [0.43.0] - 2026-09-01

### Fixed

- **Security:** `GET /v1/bridge/plugins/{id}/{*path}` authorizes the caller, it no longer merely authenticates them. It checked for a valid token and then served any plugin's bytes, so any authenticated user could pull an admin plugin's bundle by path and read skills and dashboards their signed manifest never offered. The endpoint now resolves the caller's own manifest candidate through the same `ManifestService` and marketplace filter the manifest endpoint uses, and 404s a plugin that candidate does not carry.

### Added

- The bridge host route accepts `opencode`, and validates enabled hosts against `systemprompt_models::bridge::profile::KNOWN_HOSTS` rather than its own private list.

## [0.42.0] - 2026-08-31

### Changed

- Establishing a session is bounded and degrades instead of failing. It reads and writes the database and runs as a global layer over every route including static content, so a database fault held each request for the pool's full 30-second acquire timeout and then returned `500` — for a public page view as much as for an API call. A request whose session cannot be established within two seconds is now served with an untracked, actor-less context and a throttled warning naming the path. Nothing is escalated by that context: it carries no auth token and no user, so every gate above `public` still refuses it, and `is_tracked` is false so the analytics sinks record no visit they cannot attribute. Measured against the live site with Postgres stopped: a page went from `500` after 30s to `200` in 2s.
- The health probe is bounded and reports which version answered.
- Gateway model resolution honours the server-side default model.

## [0.41.0] - 2026-08-28

### Changed

- The MCP reverse proxy no longer overwrites the request context's `agent_name` with the target server's name; the caller's own agent identity reaches the MCP server, and the callee is recorded as `server_name` as before. A2A services still take their own name as the handling agent.
- Gateway governance audits record the OAuth `client_id` from the validated bearer token.

### Fixed

- A client-supplied `x-agent-name` that fails `AgentName` validation (empty, or the reserved `unknown`) is ignored with a warning instead of panicking the request handler. A proxied service whose name fails the same validation answers `400` (`invalid_service_name`).

## [0.40.0] - 2026-08-26

### Added

- OpenAI Chat Completions inbound surface: `POST /v1/chat/completions` (canonical parse + `chat.completion` / SSE render + OpenAI error envelopes), `/v1/models?format=openai`, and a byte-preserving raw outbound lane for chat-to-chat relays. `stream_tap` renders terminal frames exactly once when a provider emits `MessageStop` more than once.

### Changed

- Secret-scan denials carry per-part provenance from `GovernedInput::Prompt { parts }` — a denial names the leaf that matched (`system`, `messages[i].<role>`, `forwarded.<leaf.path>`) instead of `prompt.text`.

## [0.39.0] - 2026-08-25

### Added

- Gateway dispatch enforces route `requires:` governance (`european`/`no_retain`): an unsatisfied requirement is denied pre-dispatch with a policy audit record, and the route-match descriptor carries `requires:<flags>`. The dispatch-time check is what covers selector-refined routes and models absent from the registry, which the boot-time validation in `systemprompt-models` cannot see.

## [0.38.0] - 2026-08-25

### Added

- `PreparedDispatch`/`GovernedDispatch`/`ScannedDispatch` in `services/gateway/service/stages.rs` make the dispatch order — build wire payload, govern, scan, send — reachable only in sequence.

### Changed

- **Breaking:** `SafetyScannerRegistry::get(name)` is `create(name, &SafetyConfig)` and returns an owned scanner. `heuristic` is a registered built-in rather than a special case in `finalize`, `names()` is sorted, and an extension registering under a built-in's name is rejected and logged instead of silently shadowing it.

## [0.33.0] - 2026-08-20

### Added

- The OAuth authorize endpoint redirects to `security.login_page_url` (when configured) with the original query string instead of rendering the built-in WebAuthn form; `prompt=passkey` bypasses the redirect and renders the built-in form.

## [0.32.2] - 2026-08-19

### Fixed

- Gateway request logs record the client-visible path including the router prefix instead of the stripped nested path.

## [0.32.0] - 2026-08-19

### Changed

- `GET /v1/bridge/manifest` and `POST /v1/bridge/enabled-hosts` respect instance-level host gating: a host whose `external_agents` catalog entry sets `enabled: false` is omitted from the manifest's enabled hosts and cannot be enabled per-user (the endpoint returns `422`).

## [0.31.0] - 2026-08-18

### Changed

- `GET /v1/bridge/manifest` returns a `SignedManifestEnvelope`: the JCS-canonical manifest string as `payload` plus the signature over those exact bytes, making manifest fields forward-compatible for older bridges.
- The builtin `heuristic` safety scanner is constructed per policy from `safety.heuristic`, so each policy's phrase list applies; extension scanners registered under the same name still shadow it.

## [0.30.0] - 2026-08-07

### Added

- The signed bridge manifest carries `allow_claude_ai_connectors`, read from the services config's `bridge_policy:` block at manifest assembly, so an instance can re-allow claude.ai first-party connectors under the bridge's managed-MCP enforcement.
- `/v1/bridge/latest` and `/v1/bridge/download/{platform}` serve the bridge self-updater from the `gateway.bridge_releases` feed. Release assets live in a private repo the bridge holds no credential for, so the gateway resolves and proxies them; the advertised digest is read from the release's signed SHA256SUMS, never recomputed. An unconfigured gateway answers 404.

## [0.29.0] - 2026-08-04

### Fixed

- The WebAuthn registration endpoints (`register/start`, `register/finish`) honour `security.allow_registration` and return 403 `registration_disabled` when it is off. The flag previously hid the authorize page's register link but left the user-creating endpoints open.

### Added

- Gateway audit extracts the offered tool definitions from the request body into `ai_request_payloads.offered_tools`, matching the in-process inference path.
- `/health` reports `degraded` with `events: { "relay": "not_listening" }` when the cross-replica event relay has lost its listener. A replica in that state serves HTTP correctly while silently dropping every event originating on another replica, which previously showed as `healthy`.

### Fixed

- The event bridge takes the write pool rather than the read pool. `LISTEN`/`NOTIFY`, the outbox insert, and the retention prune all require the primary, so a deployment with a separate read URL pointed at a standby started a relay that could never listen. `write_pool()` falls back to the read pool when no write URL is configured, so a single-pool deployment is unaffected.

## [0.28.0] - 2026-07-31

### Breaking

- **Breaking:** `gateway::pricing::cost_microdollars` takes a `CostTokens` struct rather than two `u32` arguments, so the four same-typed counts cannot be silently transposed at a call site.

### Added

- The `urn:ietf:params:oauth:grant-type:jwt-bearer` grant (RFC 7523) redeems an ID-JAG for an access token, the redemption leg of MCP Enterprise-Managed Authorization. It shares its validator with the equivalent token-exchange call, which stays available, and both `/.well-known/oauth-authorization-server` and the per-server MCP metadata advertise the `urn:ietf:params:oauth:grant-profile:id-jag` profile so an EMA-capable client knows to present an ID-JAG rather than redirect the user.
- A token exchange is pinned to the resource its ID-JAG names. Without the binding an ID-JAG obtained for one MCP server could be redeemed against any other resource in the deployment's allowlist; the request may now omit `resource` or name the same one, and anything else is rejected. An ID-JAG minted without the claim still leaves the choice to the request, which keeps grants issued before this change redeemable.
- An ID-JAG exchange issues its token for the employee the assertion names — linked to a local account by `(iss, sub)` — rather than for the client's owner.

### Fixed

- Cache reads and cache writes are billed at their own rates instead of being ignored. For an agent loop resending a large cached prompt they are the bulk of the tokens, so costing on `input + output` alone understated the bill by an order of magnitude; `tokens_used` now counts all four buckets too.
- A streamed request is recorded as having reported usage only when a real usage event arrived. The `message_start` snapshot carries small non-zero placeholders, so token counts alone could not distinguish "billed usage reported" from "never reported", and the stream tap accepted the placeholder as the final answer.
- The stream tap takes the last usage snapshot outright rather than keeping the largest value per field. Every producer emits a complete cumulative snapshot, so the field-wise `> 0` guards were wrong twice over: they let a stale `message_start` estimate survive a real later zero, and they left `total_tokens` — which no producer sets on a delta — permanently stale.
- The stream tap merges a usage update rather than replacing its whole snapshot, so a frame stating only `output_tokens` no longer zeroes the input and cache counts `message_start` established. A count the frame does state wins even at zero, so a stale estimate still cannot survive a real later report. The Anthropic inbound render omits a count the upstream frame never stated instead of emitting it as zero.
## [0.27.0] - 2026-07-29

### Changed

- Gateway request guards receive the resolved request (requested model, route id, provider, streaming flag) and a `Forbidden` denial now renders a 403 without `retry-after`; quota-kind denials keep the 429 + `retry-after` path. Guard denials were previously all funnelled through the quota response, so an entitlement denial invited clients to retry forever.
- Gateway quota windows resolve their bucket subject per `QuotaWindow.subject`: `user` (default) keys on the requesting user; an extension dimension (e.g. `organization`) resolves through the registered `SubjectAttributeProvider`, first value wins, and a window whose subject cannot be resolved is skipped. `precheck_and_reserve` additionally denies once a window's `max_cost_microdollars` ceiling is spent, and `post_update_tokens` records the request cost computed by the audit (`GatewayAudit::complete` returns it).

- WebAuthn registration promotes a prior anonymous session's full history onto the new account via `UserService::promote_anonymous` (transactional, all user-data tables) instead of moving `user_sessions` rows alone. Promotion stays best-effort: registration never fails on a merge error, which remains repairable via `admin users merge`.

### Fixed

- Streaming completions debit quota buckets (tokens and the cost `GatewayAudit::complete` computes) and run the response-phase safety scanners; both were buffered-only, so streamed traffic bypassed token counters, `max_cost_microdollars` ceilings, and response-phase findings. A failed stream debits only its precheck reservation.
- Static assets whose filename carries no content hash are served `Cache-Control: public, max-age=0, must-revalidate`, the new `CACHE_STATIC_ASSET_REVALIDATE`. `asset_cache_policy` reserves `CACHE_STATIC_ASSET` for hashed names such as `app.4f3a9c1e.js` or `main-8ba7f21c.css`; `immutable` suppresses revalidation, so the `ETag` on these responses was unreachable and a redeployed asset could stay cached for up to a year. Files under the configured files prefix honour `files.cacheControl` when set.

## [0.26.0] - 2026-07-28

### Fixed

- The Anthropic-protocol inbound parser drops content blocks it does not model (`redacted_thinking`, `document`, `server_tool_use`, `web_search_tool_result`) instead of rejecting the whole request with a 400 — mirroring the response-side parse, which strips the same types before a client ever sees them. A client replaying history from a direct Anthropic session (web search especially) now degrades instead of failing. Unknown roles still reject.
- The OpenAI Responses inbound adapter round-trips reasoning items: the provider `id` and `encrypted_content` a client sends are parsed into the canonical model (an encrypted-only item with an empty summary is kept rather than vanishing), and rendered reasoning items carry the real upstream id on all three render paths — buffered, block-start, and terminal — falling back to the synthetic `rs_…` only when no provider id exists. The stream accumulator captures `encrypted_content` arriving at `output_item.done`, so terminal renders and the audit trail carry it.
- Gemini multi-turn tool calls survive strict Anthropic clients. Gemini's `thoughtSignature` must be echoed back verbatim on the turn after a function call, and the gateway carried it to the client as a non-standard `signature` field on the `tool_use` block — a channel any faithful Anthropic SDK client strips when replaying history, at which point Gemini rejects the signatureless replay. The new `ThoughtSignatureCache` captures signatures server-side as responses pass through (buffered and streamed alike) and re-injects them on inbound requests whose `tool_use` blocks arrive without one; a client that does round-trip the field still wins over the cache. Entries are keyed by conversation and `tool_use` id — the ids on inbound requests are client-supplied, so an unscoped key would let one caller read another conversation's cached signatures — and expire an hour after last use. The cache is per-process; multi-replica gateways need sticky routing for signature recovery to hit.

## [0.25.0] - 2026-07-27

### Changed

- Static assets, marketplace plugin files, and bridge plugin files all resolve their type through `systemprompt_models::mime` rather than three local tables. JavaScript is served as `text/javascript` rather than `application/javascript`, YAML as `application/yaml` rather than `text/yaml`, and served `text/*` responses — HTML and CSS included — now carry `charset=utf-8`.

### Fixed

- One safety finding no longer denies the rest of a conversation. `enforce_request_safety` matched `block_categories` against a bare category string with no notion of which turn raised the finding, so anything the built-in scanner found in the conversation history denied the current request. Blocking is now phase-aware: only a finding against the newest turn denies, unless `safety.history` is set to `block`.
- Duplicate `ai_safety_findings` rows are collapsed. A scanner emits one finding per match, so a message tripping two jailbreak phrases wrote two rows; findings are deduplicated by phase, category and scanner before persistence, on both the request and response paths.
- Gateway requests are attributed to their caller in the `logs` table. The gateway router is nested without the context middleware — it authenticates inside the handler — so the access log ran before any principal existed and recorded every request as the platform owner. A single rejected request read back as two users acting at the same instant. The handler hands the resolved principal back on the response and the access log uses it, falling back to the platform actor only when the request never authenticated. These rows carry `"kind": "access_log"`.
- A request rejected before it authenticated no longer warns about a skipped audit row. It has no principal by construction, and forcing an `ai_requests` row would let anything probing `/v1/messages` write unbounded rows; the access-log entry records the rejection and its status, and the message drops to `DEBUG`.
- The rejection record writes `NULL` for provider and model rather than the literal `"unknown"`, and is stamped `rejected`.
- The `null` scanner resolves from the gateway scanner registry. It was exported and documented as the scanner to name when scanning is disabled but never registered, so naming it silently ran nothing.
- Static assets with a `webp`, `gif`, or `avif` extension are served with their image type instead of `application/octet-stream`. Core already classified `.webp` as a static asset, accepted `image/webp` on upload, and recorded it as such, but the serving table could not name the type it had just ingested. Because the same response sets `x-content-type-options: nosniff`, the browser is forbidden from recovering by sniffing, so such an image returned a clean 200 and silently never decoded.
- `.woff` is served as `font/woff`. It was served as `font/woff2`, a different format.
- Shutdown no longer strands MCP and agent child processes. The forced-exit deadline was armed when the first signal landed, which is *before* axum begins draining connections, so a long-lived SSE stream could consume the whole grace window and kill the process before any child was signalled. The connection drain is now bounded separately and abandoned on expiry, and the hard deadline is armed only once the drain has returned, so child termination always gets its full grace. A second signal still exits immediately.
- The scheduler startup event reports the configured job count alongside the discovered one.

## [0.24.0] - 2026-07-26

### Changed

- Scheduler initialisation failure is fatal: the server aborts startup instead of logging a warning and continuing. `run_bootstrap_jobs` runs only in that phase, so a rejected scheduler config previously left every boot-time job unexecuted while the process reported a successful start and passed its health check. Disable the scheduler with `scheduler.enabled: false`, which still starts cleanly.

## [0.23.0] - 2026-07-24

### Breaking

- **Breaking:** `POST /api/v1/sync/files` (the services-tree upload) is removed together with the cloud-sync feature; the tarball extraction helpers behind it are gone. `GET /api/v1/sync/files` and `GET /api/v1/sync/files/manifest` remain and back the new `systemprompt cloud backup` command.

### Added

- The client-IP resolver honours `Fly-Client-IP` under the same trusted-peer gate as `X-Real-IP` and `CF-Connecting-IP`.
- The session middleware logs an hourly-throttled warning when an untrusted private-range peer presents `X-Forwarded-For`, the signature of a proxy missing from `server.trusted_proxies`.

## [0.22.0] - 2026-07-20

### Added

- `ClientIp` request extractor and `resolve_client_ip*` helpers centralise originating-IP resolution against the trusted-proxy allowlist.

### Changed

- Session, OAuth token, and bridge handlers resolve the client IP once at the HTTP boundary and pass it into session analytics.
- The session middleware extracts request analytics exactly once per request and passes it down by reference. Establishing a session previously re-derived it — repeating the user-agent parse, referrer parse, and GeoIP lookup — two or three times.
- `X-Frame-Options` is emitted from the typed `FrameOptions` config and can be disabled.

### Fixed

- `user_sessions.ip_address` records the trusted-proxy-attested client IP instead of a spoofable hop-header value.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.21.0] - 2026-07-16

### Breaking

- `ApiServer` and `ServerConfig` are removed; `setup_api_server` returns the composed `axum::Router` and `run_server` takes the pre-bound listener. Migrate by calling `services::server::bind_and_serve(addr, events)` before context construction and passing the returned `EarlyServer` to `run_server`.

### Changed

- The server binds its TCP listener before bootstrap: `/api/v1/health` and `/health` return `200 {"status":"starting"}` (all other routes `503`) while migrations, content publish, and agent reconciliation run, then the full router is swapped onto the same listener. Platform health checks no longer fail during slow first boots; a bootstrap failure still exits non-zero with the listener closed. `is_ready`/`wait_for_ready` signal when the full router activates.

## [0.20.0] - 2026-07-15

### Changed

- `POST /v1/agent/contexts` creates contexts with `ContextKind::User`; CLI bookkeeping rows no longer appear in context listings backed by conversation analytics.

## [0.19.0] - 2026-07-02

### Added

- `GET /v1/bridge/manifest` includes the signed `artifacts` section of Cowork Artifacts-library documents.

## [0.18.0] - 2026-07-01

### Added

- External MCP servers are reachable over the MCP HTTP protocol: `POST /api/v1/mcp/{name}/mcp` mints a per-user provider bearer server-side and forwards the request to the provider, without exposing the provider URL or token to the client. Client-mediated `tools/call` requests are recorded per-user in the tool-execution audit.

## [0.17.0] - 2026-06-24

### Added

- Slack, Teams, and messaging gateway routes that verify inbound chat requests and dispatch them to A2A agents under the standard authorization pipeline.
- `/v1/auth/bridge/session-pat` route minting a durable personal access token from the one-time bridge exchange code, plus device-PAT issuance.
- `GET /health` reports `"status": "degraded"` (HTTP 200) with a `scheduler.degraded_jobs` list when the scheduler skipped jobs at startup because their configured owner did not resolve.

### Changed

- The gateway captures cache-token usage (`cache_read_tokens`, `cache_creation_tokens`) from buffered and streaming provider responses and records it on the request's audit row, so cache hits and their token counts are now persisted alongside input/output totals.
- Gateway route resolution evaluates a route's `when` request-shape predicates and any registered `RouteSelector`, and records how the route was chosen — the matched predicates and/or the selector that fired — in the new `ai_requests.route_match` audit column.

## [0.16.1] - 2026-06-22

### Added

- The token endpoint issues and consumes ID-JAG assertions via RFC 8693 token exchange (Enterprise-Managed Authorization), with single-use replay rejection.
- OAuth and MCP discovery metadata advertise `subject_token_types_supported` and `issued_token_types_supported`, and MCP protected-resource metadata advertises the EMA extension for resource-bound servers.

### Fixed

- A managed service's configured `audience` is now enforced against the caller's token; previously it was declared but not checked.
- API startup no longer aborts when an external MCP server is enabled: reconciliation counts only the servers core spawns toward the running-process total, so an external (remote) server no longer registers as a missing required service.

## [0.16.0] - 2026-06-22

### Breaking

- Error enum tuple variants that wrapped a bare message string are now struct variants with a named `message` field; match arms and constructors change from `Error::Foo(msg)` to `Error::Foo { message: msg }`.
- The minimum supported Rust version is 1.88.

### Changed

- Context-webhook event loading and notification handling return typed errors; failure modes map to specific HTTP statuses (400/404) instead of a blanket 500.
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
