# systemprompt-teams

[![Crates.io](https://img.shields.io/crates/v/systemprompt-teams.svg?style=flat-square)](https://crates.io/crates/systemprompt-teams)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-teams?style=flat-square)](https://docs.rs/systemprompt-teams)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Microsoft Teams integration for [systemprompt.io](https://systemprompt.io).

Every Teams message answers to the same audit pipeline as every other surface.
Bot Framework activities (messages and invokes) delivered through the Azure Bot
Service are token-verified, mapped to governed systemprompt identities,
authorized against RBAC, dispatched to A2A agents, and answered back in Teams,
under one governed path.

This crate is **fully opt-in**: it is excluded from the facade's `default` and
`full` feature sets and only compiles under the `teams` feature.

## How Teams differs from Slack

Microsoft does not use a static signing secret or bot token. Instead:

- **Inbound** activities carry an `Authorization: Bearer <JWT>` issued by the Bot
  Connector. The token is validated against the Bot Connector OpenID metadata
  (issuer `https://api.botframework.com`, audience = the bot's Microsoft App Id),
  with signing keys fetched from the published JWKS and cached.
- **Outbound** replies require an OAuth2 client-credentials access token (against
  `login.microsoftonline.com/botframework.com`, scope
  `https://api.botframework.com/.default`), then a POST to the activity's
  `serviceUrl`.

There is no official Microsoft Bot Framework SDK for Rust; the wire is
implemented here from scratch over `jsonwebtoken` and `reqwest`.

## Configuration

Teams apps are declared declaratively in `services/teams/<name>.yaml`:

```yaml
tenant_id: "00000000-0000-0000-0000-000000000000"
app_id: "11111111-1111-1111-1111-111111111111"   # Microsoft App (bot) id
app_password_ref: "teams_app_password"            # resolved from profile secrets
enabled: true
default_agent: "support-agent"
routing:
  "19:meeting_abc@thread.v2": "triage-agent"
  "/ask": "qa-agent"
authz:
  allowed_roles: ["teams-user"]
```

Secrets are referenced by name and resolved from the profile secret store —
never inlined.

## License

BUSL-1.1
