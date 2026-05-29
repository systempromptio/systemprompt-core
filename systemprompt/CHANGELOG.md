# Changelog

## [0.13.0] - 2026-05-29

The `systemprompt` facade tracks the workspace version. This release re-exports the 0.13.0 surface of every member crate; consult the root `CHANGELOG.md` and per-crate changelogs for behavioural changes. Notable highlights surfaced through the facade:

- `systemprompt::security` JWT validation is consolidated onto a single RS256 decode primitive behind a `ValidationPolicy`; request-context middleware, session, hook-token, and the OAuth / MCP / agent domains all route through it. JTI revocation now runs inside the JWT context extractor as the final stateful check and fails closed.
- `systemprompt::security::AuthzRequest` carries `session_id: Option<SessionId>`, threaded into the authz audit row's `session_id` column. Agent task and artifact routes verify context ownership before returning rows.
- `systemprompt::security::ValidatedHookClaims` fields are typed (`PluginId` / `UserId`), and the gateway capture/audit path uses `AiToolCallId`. The default session-cookie name is centralised on `CookieExtractor::DEFAULT_COOKIE_NAME`.
- `systemprompt::models::env::{read_env_optional, interpolate, contains_placeholder}` is the single `${VAR}` / `${VAR:-default}` interpolation primitive shared by the profile loader and the services config layer.

Feature flags are unchanged: `core` (default), `database`, `api`, `cli`, `full`.

## [0.13.0] - 2026-05-28

The `systemprompt` facade tracks the workspace version. This release re-exports the 0.13.0 surface of every member crate; consult the root `CHANGELOG.md` and per-crate changelogs for behavioural changes. Notable highlights surfaced through the facade:

- `systemprompt::oauth::DynamicRegistrationRequest::{get_grant_types, get_response_types}` now apply the RFC 7591 §2 server defaults (`["authorization_code"]` / `["code"]`) when the dynamic-client-registration payload omits the field, returning `Vec<String>` infallibly. Spec-compliant MCP clients (Cowork, Claude Code DCR, MCP Inspector) no longer hit `400 invalid_client_metadata` on minimal registration payloads.
- `systemprompt::mcp::Deployment.endpoint` is now `Option<String>` and, for `internal` servers, must be relative; the gateway derives the public URL from `api_external_url`.
- `systemprompt::models::PluginComponentRef` is now the uniform shape for every entity-id reference list across `MarketplaceConfig`, `PluginConfig`, `SkillConfig`, `DiskAgentConfig`, `AgentMetadataConfig`, and bridge manifest entries. Flat-list YAML is rejected.
- `systemprompt::api::services::proxy::auth::OAuthChallengeBuilder` derives the `WWW-Authenticate: Bearer resource_metadata="…"` URL from the incoming `Host` header, closing the host-of-truth gap on RFC 9728 dual-self-identity gateways.
- `systemprompt::security::AuthMode` is removed; `AuthValidationService::validate_request` takes only the headers. Anonymous routes wire the public middleware flavour.
- `systemprompt::identifiers::bootstrap::{anonymous, bot, unknown, default, empty_sentinel}` and the `UserId::{anonymous, system, bootstrap}` constructors are deleted. `UserId` values must originate from a row in the `users` table.

Feature flags are unchanged: `core` (default), `database`, `api`, `cli`, `full`.

## [0.12.1] - 2026-05-27

### Fixed

- `systemprompt::api::services::server::metrics::install_recorder` is now idempotent: the `PrometheusHandle` is cached in a process-wide `OnceLock` so repeat callers (test binaries booting multiple `ApiServer`s, future hot-reload paths) get a clone of the original handle instead of failing with "attempted to set a recorder after the metrics system was already initialized".

## [0.12.0] - 2026-05-27

The `systemprompt` facade tracks the workspace version. This release re-exports the 0.12.0 surface of every member crate; consult the root `CHANGELOG.md` and per-crate changelogs for behavioural changes. Notable highlights this release surface through the facade:

- `systemprompt::models::JwtClaims`, `AuthzRequest`, `JwtUserContext`, and `SessionParams` lose `department` in favour of an `attributes: BTreeMap<String, serde_json::Value>` bag. Every JWT issued by a pre-0.12 build is incompatible with this release; rotate signing keys or wait out existing token lifetimes before upgrading.
- `systemprompt::security::AuthzContext` is now a `{ kind, payload }` carrier with `AuthzContext::{none, gateway_invocation, mcp_tool_call, extension}` constructors. Pattern matches on the prior `AuthzContext::GatewayInvocation { model }` shape no longer compile.
- `systemprompt::security::RuleBasedHook` promotes the core RBAC resolver to a first-class `AuthzDecisionHook`. Bootstrap composes `[RuleBasedHook, ...extensions]` automatically when a DB pool is available.
- `systemprompt::models::Profile::gateway` splits into `GatewayConfigSpec` (on-disk) and `GatewayConfig` (runtime), gated through `GatewayState`. The on-disk `gateway.catalog_path:` field is replaced by `gateway.catalog:` (file-backed or inline).
- `systemprompt::models::AccessControlConfig.departments` and `RuleEntry.departments` are gone. Migration `008_drop_department_acl.sql` narrows `access_control_rules.rule_type` to `('role','user')` and deletes existing department rows.

Feature flags are unchanged: `core` (default), `database`, `api`, `cli`, `full`.

## [0.11.0] - 2026-05-20

The `systemprompt` facade tracks the workspace version. This release re-exports the 0.11.0 surface of every member crate; consult the root `CHANGELOG.md` and per-crate changelogs for behavioural changes. Notable highlights this release surface through the facade:

- `systemprompt::security::TokenAuthority`, the published `/.well-known/jwks.json` set, and RS256-only token validation replace the prior HS256 path.
- `systemprompt::oauth::GrantType::TokenExchange` and the new RFC 8693 grant on `/oauth/token`, with `ActClaim` propagated end-to-end.
- `systemprompt::identifiers::Actor` and `ActorKind`, threaded through `ToolContext` and `JobContext` so every audit-bearing row carries an accountable principal.
- `systemprompt::models::JwtAudience::Bridge` (renamed from `Cowork`); persisted tokens with `aud: "cowork"` no longer validate.

Feature flags are unchanged: `core` (default), `database`, `api`, `cli`, `full`.
