# systemprompt-slack

Slack integration for [systemprompt.io](https://systemprompt.io).

Turns Slack into a first-class inbound surface alongside the gateway and MCP.
Slack Events API messages, slash commands, and Block Kit interactions are
signature-verified, mapped to governed systemprompt identities, authorized
against RBAC, dispatched to A2A agents, and answered back in Slack — under the
same audit pipeline as every other surface.

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

Secrets are referenced by name and resolved from the profile secret store —
never inlined.

## License

BUSL-1.1
