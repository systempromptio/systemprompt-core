<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**CLI Reference**](https://github.com/systempromptio/systemprompt-core/tree/main/crates/entry/cli) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---


# Config CLI Commands

`admin config` reads and edits the active profile YAML, the single source of truth for how the instance runs. Every group below writes back to that profile. All commands support non-interactive mode for automation.

---

## Prerequisites

```bash
export SYSTEMPROMPT_PROFILE=~/.systemprompt/profiles/local/profile.yaml
cd /var/www/html/systemprompt-core
cargo build --package systemprompt-cli

alias sp="./target/debug/systemprompt --non-interactive"
```

---

## Command Groups

`admin config` has thirteen groups. `show`, `list`, and `validate` are top-level; the rest are subcommand groups.

| Group | Purpose |
|-------|---------|
| `show` | Configuration overview for the active profile |
| `list` | List all configuration files |
| `validate` | Validate configuration files (exits non-zero on failure) |
| `server` | Host, port, HTTPS, and CORS origins |
| `runtime` | Environment, log level, output format |
| `security` | JWT issuer, token expiry, trusted issuers |
| `paths` | Show and validate configured filesystem paths |
| `provider` | Enable/disable AI providers and set the default (policy) |
| `gateway` | Gateway enable/disable, routes, default provider |
| `governance` | Authorization hook (webhook/extension/disabled/unrestricted) |
| `catalog` | Provider registry under `profile.providers` (providers, models) |
| `secret` | Set a provider or custom secret in the secrets store |
| `rate-limits` | Per-endpoint rates, tier multipliers, presets, import/export |

---

## show / list / validate

```bash
sp admin config show              # overview of the active profile
sp admin config list              # configuration files on disk
sp admin config validate          # validate; exits non-zero if invalid
```

`show` returns a `Card`, `list` and `validate` return a `Table`. `validate` bails with a non-zero exit code when the profile is invalid.

---

## server

Host, port, HTTPS, and CORS origins.

```bash
sp admin config server show
sp admin config server set --host 127.0.0.1 --port 8080 --use-https false
sp admin config server cors list
sp admin config server cors add https://app.example.com
sp admin config server cors remove https://app.example.com
```

| Subcommand | Purpose |
|------------|---------|
| `show` | Show server configuration |
| `set` | Set `--host`, `--port`, `--use-https` |
| `cors list` / `cors add <ORIGIN>` / `cors remove <ORIGIN>` | Manage allowed CORS origins |

---

## runtime

Environment, log level, and output format.

```bash
sp admin config runtime show
sp admin config runtime set --environment production --log-level normal --output-format json
```

| Subcommand | Purpose |
|------------|---------|
| `show` (alias `list`) | Show runtime configuration |
| `set` | Set `--environment`, `--log-level`, `--output-format` |

---

## security

JWT issuer, token expiry, and trusted issuers for token exchange.

```bash
sp admin config security show
sp admin config security set --jwt-issuer https://auth.example.com --access-expiry 3600 --refresh-expiry 2592000
sp admin config security trusted-issuer add \
  --issuer https://idp.example.com \
  --jwks-uri https://idp.example.com/.well-known/jwks.json \
  --audience systemprompt
sp admin config security trusted-issuer remove --issuer https://idp.example.com
```

| Subcommand | Purpose |
|------------|---------|
| `show` (alias `list`) | Show security configuration |
| `set` | Set `--jwt-issuer`, `--access-expiry`, `--refresh-expiry` |
| `trusted-issuer add` / `remove` | Manage RFC 8693 token-exchange issuers |

---

## paths

Show and validate the configured filesystem paths.

```bash
sp admin config paths show
sp admin config paths validate
```

| Subcommand | Purpose |
|------------|---------|
| `show` (alias `list`) | Show configured paths |
| `validate` | Verify every configured path exists |

---

## provider

Enable, disable, and set the default AI provider. This is the policy layer over the catalog: it decides which declared providers are usable and which is the default.

```bash
sp admin config provider list
sp admin config provider enable anthropic
sp admin config provider disable openai
sp admin config provider set anthropic
```

| Subcommand | Purpose |
|------------|---------|
| `list` | List AI providers |
| `set <PROVIDER>` | Set the default provider |
| `enable <PROVIDER>` / `disable <PROVIDER>` | Toggle a provider |

---

## gateway

Gateway enable/disable, model routes, and default provider.

```bash
sp admin config gateway enable
sp admin config gateway disable
sp admin config gateway route list
sp admin config gateway route add --model-pattern "claude-*" --provider anthropic
sp admin config gateway route remove --model-pattern "claude-*"
sp admin config gateway default-provider set anthropic
sp admin config gateway default-provider clear
```

| Subcommand | Purpose |
|------------|---------|
| `enable` / `disable` | Toggle the gateway |
| `route list` / `route add` / `route remove` | Manage routes (upsert by `--model-pattern`, targeting `--provider`) |
| `default-provider set` / `clear` | Set or clear the gateway default provider |

---

## governance

The authorization hook that every request is evaluated against.

```bash
sp admin config governance show
sp admin config governance set --mode webhook --url https://authz.example.com/evaluate
sp admin config governance set --mode extension
sp admin config governance set --mode disabled
```

| Subcommand | Purpose |
|------------|---------|
| `show` | Show governance configuration |
| `set` | Set `--mode` (`webhook`, `extension`, `disabled`, `unrestricted`); `--url` required for `webhook` |

`unrestricted` requires an explicit acknowledgement; it disables authorization entirely.

---

## catalog

The provider registry under `profile.providers`: which providers exist and which models they serve. Providers must be declared here before `provider` or `gateway` can reference them.

```bash
sp admin config catalog provider list
sp admin config catalog provider add --name anthropic --wire anthropic
sp admin config catalog provider remove anthropic
sp admin config catalog model add --provider anthropic --id claude-sonnet-4-6-20250610 --alias sonnet
sp admin config catalog model remove --provider anthropic --id claude-sonnet-4-6-20250610
```

| Subcommand | Purpose |
|------------|---------|
| `provider list` / `provider add` / `provider remove` | Manage declared providers (`--wire`: `anthropic`, `openai-chat`, `openai-responses`, `gemini`) |
| `model add` / `model remove` | Manage models under a provider (repeatable `--alias`) |

---

## secret

Set a provider or custom secret in the profile's secrets store.

```bash
sp admin config secret set anthropic "sk-ant-..."
sp admin config secret set minimax "..."
```

| Subcommand | Purpose |
|------------|---------|
| `set <NAME> <VALUE>` | Write a secret to the profile's secrets file |

---

## Rate Limits Commands

The `rate-limits` group is the richest. Alongside `show`, `tier`, `docs`, `set`, `enable`, `disable`, `validate`, `compare`, and `reset`, it also has `preset`, `export`, `import`, and `diff`:

```bash
sp admin config rate-limits preset list
sp admin config rate-limits preset show <name>
sp admin config rate-limits preset apply <name>
sp admin config rate-limits export limits.json
sp admin config rate-limits import limits.json
sp admin config rate-limits diff --file limits.json
```

### config rate-limits show

Show current rate limit configuration from the profile.

```bash
sp admin config rate-limits show
sp --json admin config rate-limits show
```

**Output Structure:**
```json
{
  "disabled": true,
  "oauth_public_per_second": 2,
  "oauth_auth_per_second": 2,
  "contexts_per_second": 50,
  "tasks_per_second": 10,
  "artifacts_per_second": 15,
  "agent_registry_per_second": 20,
  "agents_per_second": 3,
  "mcp_registry_per_second": 20,
  "mcp_per_second": 100,
  "stream_per_second": 1,
  "content_per_second": 20,
  "burst_multiplier": 2,
  "tier_multipliers": {
    "admin": 10.0,
    "user": 1.0,
    "a2a": 5.0,
    "mcp": 5.0,
    "service": 5.0,
    "anon": 0.5
  }
}
```

**Artifact Type:** `Card`

---

### config rate-limits tier

Show effective limits for a specific tier (base rates multiplied by tier multiplier).

```bash
sp admin config rate-limits tier admin
sp admin config rate-limits tier user
sp admin config rate-limits tier anon
sp --json admin config rate-limits tier a2a
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<TIER>` | Yes | Tier name: `admin`, `user`, `a2a`, `mcp`, `service`, `anon` |

**Output Structure:**
```json
{
  "tier": "admin",
  "multiplier": 10.0,
  "effective_limits": {
    "oauth_public_per_second": 20,
    "oauth_auth_per_second": 20,
    "contexts_per_second": 500,
    "tasks_per_second": 100,
    "artifacts_per_second": 150,
    "agent_registry_per_second": 200,
    "agents_per_second": 30,
    "mcp_registry_per_second": 200,
    "mcp_per_second": 1000,
    "stream_per_second": 10,
    "content_per_second": 200
  }
}
```

**Artifact Type:** `Card`

---

### config rate-limits docs

Show comprehensive rate limits documentation including base rates, tier multipliers, and effective limits comparison.

```bash
sp admin config rate-limits docs
sp --json admin config rate-limits docs
```

**Output Structure:**
```json
{
  "base_rates": [
    {"endpoint": "OAuth Public", "rate_per_second": 2},
    {"endpoint": "Contexts", "rate_per_second": 50}
  ],
  "tier_multipliers": [
    {"tier": "Admin", "multiplier": 10.0},
    {"tier": "User", "multiplier": 1.0}
  ],
  "effective_limits": [
    {"endpoint": "Contexts", "admin": 500, "user": 50, "anon": 25}
  ],
  "burst_multiplier": 2,
  "disabled": true
}
```

**Artifact Type:** `Table`

---

### config rate-limits set

Set a rate limit value. Modifies the profile YAML file.

```bash
# Set endpoint rate
sp admin config rate-limits set --endpoint contexts --rate 100
sp admin config rate-limits set --endpoint tasks --rate 20

# Set tier multiplier
sp admin config rate-limits set --tier admin --multiplier 15.0
sp admin config rate-limits set --tier anon --multiplier 0.25

# Set burst multiplier
sp admin config rate-limits set --burst 3
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--endpoint <NAME>` | Endpoint: `oauth_public`, `oauth_auth`, `contexts`, `tasks`, `artifacts`, `agent_registry`, `agents`, `mcp_registry`, `mcp`, `stream`, `content` |
| `--rate <VALUE>` | Rate per second (requires `--endpoint`) |
| `--tier <NAME>` | Tier: `admin`, `user`, `a2a`, `mcp`, `service`, `anon` |
| `--multiplier <VALUE>` | Multiplier value (requires `--tier`) |
| `--burst <VALUE>` | Burst multiplier value |

**Output Structure:**
```json
{
  "field": "contexts_per_second",
  "old_value": "50",
  "new_value": "100",
  "message": "Updated contexts rate: 50 -> 100/s"
}
```

**Artifact Type:** `Text`

---

### config rate-limits enable

Enable rate limiting.

```bash
sp admin config rate-limits enable
sp --json admin config rate-limits enable
```

**Output Structure:**
```json
{
  "enabled": true,
  "message": "Rate limiting enabled"
}
```

**Artifact Type:** `Text`

---

### config rate-limits disable

Disable rate limiting.

```bash
sp admin config rate-limits disable
sp --json admin config rate-limits disable
```

**Output Structure:**
```json
{
  "enabled": false,
  "message": "Rate limiting disabled"
}
```

**Artifact Type:** `Text`

---

### config rate-limits validate

Validate rate limit configuration for errors and warnings.

```bash
sp admin config rate-limits validate
sp --json admin config rate-limits validate
```

**Validation Checks:**
- No zero or negative rates
- Positive tier multipliers
- Tier hierarchy: `anon < user < admin`
- Burst multiplier is reasonable (1-10x)

**Output Structure:**
```json
{
  "valid": true,
  "errors": [],
  "warnings": [
    "Rate limiting is currently DISABLED"
  ]
}
```

**Artifact Type:** `Card`

---

### config rate-limits compare

Compare effective limits across all tiers side-by-side.

```bash
sp admin config rate-limits compare
sp --json admin config rate-limits compare
```

**Output Structure:**
```json
{
  "endpoints": [
    {
      "endpoint": "Contexts",
      "admin": 500,
      "user": 50,
      "a2a": 250,
      "mcp": 250,
      "service": 250,
      "anon": 25
    }
  ]
}
```

**Artifact Type:** `Table`

---

### config rate-limits reset

Reset rate limits to default values.

```bash
# Preview changes (dry run)
sp admin config rate-limits reset --dry-run

# Reset all to defaults
sp admin config rate-limits reset --yes

# Reset specific endpoint
sp admin config rate-limits reset --endpoint contexts --yes

# Reset specific tier multiplier
sp admin config rate-limits reset --tier admin --yes
```

**Flags:**
| Flag | Description |
|------|-------------|
| `-y`, `--yes` | Skip confirmation (required in non-interactive mode) |
| `--dry-run` | Preview changes without applying |
| `--endpoint <NAME>` | Reset only this endpoint |
| `--tier <NAME>` | Reset only this tier multiplier |

**Output Structure:**
```json
{
  "reset_type": "all",
  "changes": [
    {
      "field": "contexts_per_second",
      "old_value": "100",
      "new_value": "50"
    }
  ],
  "message": "Reset 1 value(s) to defaults"
}
```

**Artifact Type:** `Table`

---

## Tier Reference

| Tier | Description | Default Multiplier |
|------|-------------|-------------------|
| `admin` | Administrative users | 10.0x |
| `user` | Authenticated users | 1.0x (baseline) |
| `a2a` | Agent-to-agent communication | 5.0x |
| `mcp` | MCP protocol requests | 5.0x |
| `service` | Internal service calls | 5.0x |
| `anon` | Anonymous/unauthenticated | 0.5x |

---

## Endpoint Reference

Default base rates (before the tier multiplier is applied). These are the built-in defaults from `RateLimitsConfig::default()`; a profile may override any of them, so `rate-limits show` reflects the loaded profile rather than these values. Effective admin limits are the base rate times the admin multiplier (10x), so `stream` defaults to 100/s base and 1000/s for admin.

| Endpoint | Description | Default Rate |
|----------|-------------|--------------|
| `oauth_public` | Public OAuth endpoints | 10/s |
| `oauth_auth` | Authenticated OAuth endpoints | 10/s |
| `contexts` | Context operations | 100/s |
| `tasks` | Task operations | 50/s |
| `artifacts` | Artifact operations | 50/s |
| `agent_registry` | Agent registry operations | 50/s |
| `agents` | Agent operations | 20/s |
| `mcp_registry` | MCP registry operations | 50/s |
| `mcp` | MCP operations | 200/s |
| `stream` | SSE streaming | 100/s |
| `content` | Content operations | 50/s |

Default burst multiplier is 3. Default tier multipliers: admin 10, user 1, a2a 5, mcp 5, service 5, anon 0.5.

---

## Error Handling

### Invalid Tier
```bash
sp admin config rate-limits tier invalid
# Error: Unknown tier: invalid. Valid tiers: admin, user, a2a, mcp, service, anon
```

### Invalid Endpoint
```bash
sp admin config rate-limits set --endpoint invalid --rate 100
# Error: Unknown endpoint: invalid. Valid endpoints: oauth_public, oauth_auth, contexts, tasks, artifacts, agent_registry, agents, mcp_registry, mcp, stream, content
```

### Missing Required Flags
```bash
sp admin config rate-limits set --endpoint contexts
# Error: --rate is required when --endpoint is specified

sp admin config rate-limits reset
# Error: --yes or --dry-run is required in non-interactive mode
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Get full rate limits as JSON
sp --json admin config rate-limits show | jq .

# Get specific tier effective limits
sp --json admin config rate-limits tier admin | jq '.effective_limits.contexts_per_second'

# Check if rate limiting is disabled
sp --json admin config rate-limits show | jq '.disabled'

# Compare all tiers
sp --json admin config rate-limits compare | jq '.endpoints[] | select(.endpoint == "Contexts")'

# Validate and check for errors
sp --json admin config rate-limits validate | jq '.errors'
```

---

## Complete Configuration Workflow

```bash
# Phase 1: View current configuration
sp --json admin config rate-limits show

# Phase 2: Validate configuration
sp --json admin config rate-limits validate

# Phase 3: Compare across tiers
sp --json admin config rate-limits compare

# Phase 4: Make changes
sp admin config rate-limits set --endpoint contexts --rate 100
sp admin config rate-limits set --tier admin --multiplier 15.0
sp admin config rate-limits enable

# Phase 5: Verify changes
sp --json admin config rate-limits show

# Phase 6: Reset if needed
sp admin config rate-limits reset --dry-run
sp admin config rate-limits reset --yes
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Destructive operations (`reset`) require `--yes` in non-interactive mode
- [x] `--dry-run` supported for preview


---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>CLI reference · Own how your organization uses AI.</sub>

</div>
