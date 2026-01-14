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
| `config rate-limits show` | Show rate limit configuration | `Card` | No |
| `config rate-limits list` | List all rate limit rules | `Table` | No |
| `config rate-limits set` | Set rate limit value | `Text` | No |

---

## Rate Limits Commands

### config rate-limits show

Show current rate limit configuration.

```bash
sp config rate-limits show
sp --json config rate-limits show
```

**Output Structure:**
```json
{
  "enabled": true,
  "global": {
    "requests_per_minute": 60,
    "requests_per_hour": 1000,
    "requests_per_day": 10000
  },
  "per_user": {
    "requests_per_minute": 30,
    "requests_per_hour": 500,
    "requests_per_day": 5000
  },
  "per_ip": {
    "requests_per_minute": 20,
    "requests_per_hour": 200,
    "requests_per_day": 2000
  }
}
```

**Artifact Type:** `Card`

---

### config rate-limits list

List all rate limit rules.

```bash
sp config rate-limits list
sp --json config rate-limits list
```

**Output Structure:**
```json
{
  "rules": [
    {
      "scope": "global",
      "window": "minute",
      "limit": 60,
      "enabled": true
    },
    {
      "scope": "global",
      "window": "hour",
      "limit": 1000,
      "enabled": true
    },
    {
      "scope": "per_user",
      "window": "minute",
      "limit": 30,
      "enabled": true
    }
  ],
  "total": 9
}
```

**Artifact Type:** `Table`
**Columns:** `scope`, `window`, `limit`, `enabled`

---

### config rate-limits set

Set a rate limit value.

```bash
sp config rate-limits set --scope global --window minute --limit 100
sp config rate-limits set --scope per_user --window hour --limit 1000
sp config rate-limits set --scope per_ip --window day --limit 5000
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--scope` | Yes | Scope: `global`, `per_user`, `per_ip` |
| `--window` | Yes | Time window: `minute`, `hour`, `day` |
| `--limit` | Yes | Request limit value |

**Output Structure:**
```json
{
  "scope": "global",
  "window": "minute",
  "limit": 100,
  "message": "Rate limit updated: global/minute = 100"
}
```

**Artifact Type:** `Text`

---

## Complete Rate Limits Configuration Flow

```bash
# Phase 1: View current configuration
sp --json config rate-limits show

# Phase 2: List all rules
sp --json config rate-limits list

# Phase 3: Update specific limits
sp config rate-limits set --scope global --window minute --limit 100
sp config rate-limits set --scope per_user --window hour --limit 1000

# Phase 4: Verify changes
sp --json config rate-limits show
```

---

## Rate Limit Scopes

| Scope | Description |
|-------|-------------|
| `global` | Applies to all requests system-wide |
| `per_user` | Applies per authenticated user |
| `per_ip` | Applies per IP address |

## Time Windows

| Window | Description |
|--------|-------------|
| `minute` | Rolling 60-second window |
| `hour` | Rolling 3600-second window |
| `day` | Rolling 86400-second window |

---

## Error Handling

### Invalid Scope

```bash
sp config rate-limits set --scope invalid --window minute --limit 100
# Error: Invalid scope 'invalid'. Valid scopes: global, per_user, per_ip
```

### Invalid Window

```bash
sp config rate-limits set --scope global --window invalid --limit 100
# Error: Invalid window 'invalid'. Valid windows: minute, hour, day
```

### Invalid Limit

```bash
sp config rate-limits set --scope global --window minute --limit -1
# Error: Limit must be a positive integer
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json config rate-limits show | jq .

# Extract specific fields
sp --json config rate-limits show | jq '.global.requests_per_minute'
sp --json config rate-limits list | jq '.rules[] | select(.scope == "per_user")'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `CliService`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
