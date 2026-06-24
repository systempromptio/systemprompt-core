# systemprompt-teams

Microsoft Teams integration for [systemprompt.io](https://systemprompt.io).

Turns Teams into a first-class inbound surface alongside the gateway, MCP, and
Slack. Bot Framework activities (messages and invokes) delivered through the
Azure Bot Service are token-verified, mapped to governed systemprompt
identities, authorized against RBAC, dispatched to A2A agents, and answered back
in Teams — under the same audit pipeline as every other surface.

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
