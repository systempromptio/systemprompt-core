<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://docs.systemprompt.io">Documentation</a></p>
</div>

---


# Config CLI Commands

This document provides complete documentation for AI agents to use the config CLI commands. All commands support non-interactive mode for automation.

---

## Prerequisites

```bash
export SYSTEMPROMPT_PROFILE=/var/www/html/tyingshoelaces/.systemprompt/profiles/local/profile.yaml
cd /var/www/html/systemprompt-core
cargo build --package systemprompt-cli

alias sp="./target/debug/systemprompt --non-interactive"
```

---

## Command Reference

| Command | Description | Artifact Type | Requires Services |
|---------|-------------|---------------|-------------------|
| `admin config rate-limits show` | Show rate limit configuration | `Card` | No |
| `admin config rate-limits tier <TIER>` | Show effective limits for a tier | `Card` | No |
| `admin config rate-limits docs` | Show rate limits documentation | `Table` | No |
| `admin config rate-limits set` | Set a rate limit value | `Text` | No |
| `admin config rate-limits enable` | Enable rate limiting | `Text` | No |
| `admin config rate-limits disable` | Disable rate limiting | `Text` | No |
| `admin config rate-limits validate` | Validate configuration | `Card` | No |
| `admin config rate-limits compare` | Compare limits across tiers | `Table` | No |
| `admin config rate-limits reset` | Reset to default values | `Table` | No |

---

## Rate Limits Commands

### config rate-limits show

Show current rate limit configuration from the profile.

```bash
sp admin config rate-limits show
sp --json config rate-limits show
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
sp --json config rate-limits tier a2a
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
sp --json config rate-limits docs
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
sp --json config rate-limits enable
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
sp --json config rate-limits disable
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
sp --json config rate-limits validate
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
sp --json config rate-limits compare
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

| Endpoint | Description | Default Rate |
|----------|-------------|--------------|
| `oauth_public` | Public OAuth endpoints | 2/s |
| `oauth_auth` | Authenticated OAuth endpoints | 2/s |
| `contexts` | Context operations | 50/s |
| `tasks` | Task operations | 10/s |
| `artifacts` | Artifact operations | 15/s |
| `agent_registry` | Agent registry operations | 20/s |
| `agents` | Agent operations | 3/s |
| `mcp_registry` | MCP registry operations | 20/s |
| `mcp` | MCP operations | 100/s |
| `stream` | SSE streaming | 10/s |
| `content` | Content operations | 20/s |

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
sp --json config rate-limits show | jq .

# Get specific tier effective limits
sp --json config rate-limits tier admin | jq '.effective_limits.contexts_per_second'

# Check if rate limiting is disabled
sp --json config rate-limits show | jq '.disabled'

# Compare all tiers
sp --json config rate-limits compare | jq '.endpoints[] | select(.endpoint == "Contexts")'

# Validate and check for errors
sp --json config rate-limits validate | jq '.errors'
```

---

## Complete Configuration Workflow

```bash
# Phase 1: View current configuration
sp --json config rate-limits show

# Phase 2: Validate configuration
sp --json config rate-limits validate

# Phase 3: Compare across tiers
sp --json config rate-limits compare

# Phase 4: Make changes
sp admin config rate-limits set --endpoint contexts --rate 100
sp admin config rate-limits set --tier admin --multiplier 15.0
sp admin config rate-limits enable

# Phase 5: Verify changes
sp --json config rate-limits show

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
