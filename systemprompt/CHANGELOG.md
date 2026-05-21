# Changelog

## [0.11.0] - 2026-05-20

The `systemprompt` facade tracks the workspace version. This release re-exports the 0.11.0 surface of every member crate; consult the root `CHANGELOG.md` and per-crate changelogs for behavioural changes. Notable highlights this release surface through the facade:

- `systemprompt::security::TokenAuthority`, the published `/.well-known/jwks.json` set, and RS256-only token validation replace the prior HS256 path.
- `systemprompt::oauth::GrantType::TokenExchange` and the new RFC 8693 grant on `/oauth/token`, with `ActClaim` propagated end-to-end.
- `systemprompt::identifiers::Actor` and `ActorKind`, threaded through `ToolContext` and `JobContext` so every audit-bearing row carries an accountable principal.
- `systemprompt::models::JwtAudience::Bridge` (renamed from `Cowork`); persisted tokens with `aud: "cowork"` no longer validate.

Feature flags are unchanged: `core` (default), `database`, `api`, `cli`, `full`.
