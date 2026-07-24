# Changelog

## [0.23.0] - 2026-07-24

### Breaking

- **Breaking:** the `sync` feature and the `systemprompt::sync` module are removed along with the `systemprompt-sync` crate; `full` no longer implies `sync`. There is no replacement — the deploy pipeline is internal to the CLI.

## [0.21.1] - 2026-07-17

### Changed
- Source files now carry a Business Source License 1.1 header referencing <https://systemprompt.io>.

## [0.19.0] - 2026-07-02

### Added

- The facade re-exports the 0.19.0 surface of every member crate. Notable through `systemprompt::api` and `systemprompt::mcp`: the signed bridge manifest carries an `artifacts` section of Cowork Artifacts-library HTML documents (`GET /v1/bridge/manifest`), scoped per marketplace and gated by owning-plugin enablement.

### Breaking

- The minimum supported Rust version is 1.94. rmcp is upgraded to 2.x (`ContentBlock` replaces the removed `Content`/`RawContent` pair in re-exported MCP signatures) and SQLx to 0.9. `MarketplaceCandidate::new` and `CatalogContent::into_parts` gain an artifact set, and typed identifiers replace raw strings across the cloud, files, and OAuth surfaces. See the root changelog for the full list.

## [0.18.0] - 2026-07-01

### Added

- The facade re-exports the 0.18.0 surface of every member crate. Notable through `systemprompt::api` and `systemprompt::mcp`: external MCP servers with an `external_auth` accessor are served over the MCP HTTP protocol (`POST /api/v1/mcp/{name}/mcp`), minting the per-user provider bearer server-side and auditing client-mediated `tools/call` requests, without exposing the provider URL or token to the client. See the root changelog for the full list.

## [0.17.1] - 2026-06-30

### Changed

- The facade re-exports the 0.17.1 surface of every member crate. Notable through `systemprompt-cli` and `systemprompt-runtime`: the `cloud deploy` preflight (and standalone `cloud doctor`) now validates each extension's service config against its schema before deploying, and `plugins validate`, `admin config validate`, and `admin session switch` return a non-zero exit code on failure. See the root changelog for the full list, including the `anyhow` 1.0.103 advisory bump.

## [0.17.0] - 2026-06-24

### Added

- `systemprompt::slack` (feature `slack`) re-exports `systemprompt-slack`: the Slack messaging surface verifies Slack Events API, slash-command, and Block Kit interaction requests and dispatches to A2A agents through the governed RBAC and audit pipeline.
- `systemprompt::teams` (feature `teams`) re-exports `systemprompt-teams`: the Microsoft Teams messaging surface verifies Bot Framework activities and replies with Adaptive Cards through the same governed dispatch path.

### Changed

- The facade re-exports the 0.17.0 surface of every member crate; see the per-crate changelogs for the `rmcp` 1.8 upgrade and `reqwest` 0.13 deduplication in `systemprompt-mcp`, the typed Slack/Teams identifiers in `systemprompt-identifiers`, the messaging identity-ingestion path in `systemprompt-security`, and the durable bridge session-PAT route in `systemprompt-api`.

## [0.16.1] - 2026-06-22

### Added

- Re-exports the ID-JAG / Enterprise-Managed Authorization surface added in `systemprompt-oauth` and `systemprompt-models` 0.16.1.

## [0.16.0] - 2026-06-22

The facade re-exports the 0.16.0 surface of every member crate; see the per-crate changelogs for the breaking removals in `systemprompt-traits`, `systemprompt-extension`, and `systemprompt-mcp`, the structured error fields across the library crates, and the `CommandContext` rework in `systemprompt-cli`. Notable highlights surfaced through the facade:

- `systemprompt::mcp::ExternalAuth` declares per-user bearer resolution for an `external` MCP server: core resolves a third-party token from a relative accessor endpoint with the user's systemprompt JWT and injects it on the outbound request, withholding the systemprompt credential. External servers are reached at their configured `remote_endpoint`.
- `systemprompt::models::split_frontmatter` is the shared line-anchored YAML frontmatter splitter consumed by content ingestion, disk sync, and static generation.
- JWT validation requires a first-party audience claim (`web`, `api`, `a2a`, or `mcp`); tokens minted without an audience are rejected.

### Breaking

- The minimum supported Rust version is 1.88.

### Changed

- The usage snippets in the crate docs and README track the workspace version.

## [0.14.0] - 2026-06-01

### Changed

- Workspace version bump; no API changes in this crate.

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

## [0.10.3] - 2026-05-18

### Changed

- A duplicate top-level re-export of `CredentialsBootstrap` was removed; it remains available as `systemprompt::credentials`.
- Unused dependencies were dropped and `tracing` is a dev-dependency for the examples.

## [0.10.2] - 2026-05-16

### Changed

- Workspace version bump; no API changes in this crate.

## [0.10.1] - 2026-05-15

### Changed

- Workspace version bump; no API changes in this crate.

## [0.10.0] - 2026-05-14

### Changed

- Workspace version bump; no API changes in this crate.

## [0.9.2] - 2026-05-12

### Changed

- Workspace version bump; no API changes in this crate.

## [0.9.1] - 2026-05-12

### Changed

- The prelude and the extension example track the schema-install pipeline overhaul in `systemprompt-database`; no new facade surface.

## [0.9.0] - 2026-05-08

### Changed

- Workspace version bump; no API changes in this crate.

## [0.8.0] - 2026-05-07

### Added

- The `systemprompt::marketplace` module (`full` feature) re-exports `systemprompt-marketplace`, including the `MarketplaceFilter` trait.

## [0.7.0] - 2026-05-06

### Changed

- Workspace version bump; no API changes in this crate.

## [0.6.1] - 2026-05-05

### Changed

- A stale prelude re-export was dropped; no new facade surface.

## [0.6.0] - 2026-05-05

### Added

- Runnable examples per major feature flag (`api`, `cli`, `database`, `extension`), compiled in CI.
- A crate-level feature matrix and expanded module docs on the docs.rs landing page.

### Changed

- The prelude is curated rather than a blanket re-export.

## [0.5.0] - 2026-05-04

### Added

- Granular per-module feature flags, so consumers can enable individual domain modules instead of `full`.

### Changed

- Bootstrap I/O re-exports follow the move from `systemprompt-models` to `systemprompt-config`.

## [0.4.2] - 2026-04-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.4.1] - 2026-04-27

### Changed

- Workspace version bump; no API changes in this crate.

## [0.4.0] - 2026-04-24

### Removed

- The dormant `GenericRepository` / `Entity` abstraction is removed from the prelude, following its deletion from `systemprompt-database`.

## [0.3.1] - 2026-04-24

### Changed

- Workspace version bump; no API changes in this crate.

## [0.3.0] - 2026-04-22

### Changed

- Workspace version bump; no API changes in this crate.

## [0.2.4] - 2026-04-20

### Changed

- Workspace version bump; no API changes in this crate.

## [0.2.3] - 2026-04-20

### Changed

- Workspace version bump; no API changes in this crate.

## [0.2.2] - 2026-04-17

### Changed

- Feature wiring was updated in a workspace-wide feature-flag sweep; no surface change.

## [0.2.1] - 2026-04-15

### Changed

- The crate description was updated; no API change.

## [0.2.0] - 2026-04-15

### Changed

- Workspace version bump; no API changes in this crate.

## [0.1.24] - 2026-04-14

### Changed

- Workspace version bump; no API changes in this crate.

## [0.1.23] - 2026-04-14

### Changed

- Workspace version bump; no API changes in this crate.

## [0.1.22] - 2026-04-07

### Changed

- Workspace version bump; no API changes in this crate.

## [0.1.21] - 2026-04-02

### Changed

- Workspace version bump; no API changes in this crate.

## [0.1.20] - 2026-04-01

### Changed

- Workspace version bump; no API changes in this crate.

## [0.1.19] - 2026-03-31

### Changed

- The `full` feature enables the `cli` display sinks in `systemprompt-logging`; the runtime and generator dependencies enable their `geolocation` and `image-processing` features.

## [0.1.18] - 2026-03-27

### Changed

- The crate builds with the Rust 2024 edition.
