# Changelog

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
