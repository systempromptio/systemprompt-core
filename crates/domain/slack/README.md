# systemprompt-slack

[![Crates.io](https://img.shields.io/crates/v/systemprompt-slack.svg?style=flat-square)](https://crates.io/crates/systemprompt-slack)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-slack?style=flat-square)](https://docs.rs/systemprompt-slack)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Slack integration for [systemprompt.io](https://systemprompt.io).

Every Slack message answers to the same audit pipeline as every other surface.
Slack Events API messages, slash commands, and Block Kit interactions are
signature-verified, mapped to governed systemprompt identities, authorized
against RBAC, dispatched to A2A agents, and answered back in Slack, under one
governed path.

## Configuration

Slack apps are declared declaratively in `services/slack/<name>.yaml`:

```yaml
workspace_id: "T0123456789"
signing_secret_ref: "slack_signing_secret"   # resolved from profile secrets
bot_token_ref: "slack_bot_token"
enabled: true
default_agent: "support-agent"
routing:
  "C0ABC": "triage-agent"
  "/ask": "qa-agent"
authz:
  allowed_roles: ["slack-user"]
```

Secrets are referenced by name and resolved from the profile secret store,
never inlined.

## License

BUSL-1.1
