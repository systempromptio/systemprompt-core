# Config CLI Tech Debt & Enhancements

## Overview

This document tracks the tech debt fixes and enhancements for the `config` CLI commands, specifically the `rate-limits` subcommands.

---

## Completed Fixes

### 1. Output Types Missing Required Derives

**Location:** `crates/entry/cli/src/commands/config/types.rs`

**Problem:** Output types only derived `Serialize`, missing `Deserialize` and `JsonSchema` required for MCP wrapper schema generation.

**Fix:** Added required derives to all output types:
```rust
// Before:
#[derive(Debug, Clone, Copy, Serialize)]
pub struct RateLimitsOutput { ... }

// After:
#[derive(Debug, Clone, Copy, Serialize, Deserialize, JsonSchema)]
pub struct RateLimitsOutput { ... }
```

**Types Fixed:**
- `RateLimitsOutput`
- `TierMultipliersOutput`
- `TierEffectiveLimitsOutput`
- `EffectiveLimitsOutput`

---

### 2. Forbidden Pattern: `println!` Usage

**Location:** `crates/entry/cli/src/commands/config/rate_limits.rs`

**Problem:** `execute_docs()` used 30+ `println!` calls with `#[allow(clippy::print_stdout)]`, violating CLI standards.

**Fix:**
- Removed all `println!` calls
- Created structured output types (`RateLimitsDocsOutput`, `BaseRateRow`, `TierMultiplierRow`, `EffectiveLimitRow`)
- Used `CommandResult::table()` with proper artifact type

---

### 3. Missing `config: &CliConfig` Parameter

**Location:** `crates/entry/cli/src/commands/config/rate_limits.rs:124`

**Problem:** `execute_docs()` did not accept `&CliConfig`, causing `--json` flag to be ignored.

**Fix:** Added config parameter:
```rust
// Before:
pub fn execute_docs() -> Result<()>

// After:
pub fn execute_docs(config: &CliConfig) -> Result<()>
```

---

### 4. README Documentation Mismatch

**Location:** `crates/entry/cli/src/commands/config/README.md`

**Problem:** README documented non-existent commands (`list`, `set`) and omitted actual commands (`tier`, `docs`).

**Fix:** Complete README rewrite documenting actual commands:
- `config rate-limits show` - Show current configuration
- `config rate-limits tier <TIER>` - Show effective limits for a tier
- `config rate-limits docs` - Show comprehensive documentation

---

### 5. False Compliance Checklist

**Location:** `crates/entry/cli/src/commands/config/README.md`

**Problem:** Checklist claimed compliance with items that were false.

**Fix:** Updated checklist to accurately reflect implementation state.

---

## Enhancements

### Enhancement 1: `config rate-limits set`

**Purpose:** Modify rate limit values without editing YAML directly.

**Commands:**
```bash
# Set base rate for an endpoint
sp config rate-limits set --endpoint contexts --rate 100

# Set tier multiplier
sp config rate-limits set --tier admin --multiplier 15.0

# Set burst multiplier
sp config rate-limits set --burst 3
```

**Required Flags:**
| Flag | Description |
|------|-------------|
| `--endpoint <NAME>` | Endpoint to modify |
| `--rate <VALUE>` | New rate per second |
| `--tier <NAME>` | Tier to modify multiplier |
| `--multiplier <VALUE>` | New multiplier value |
| `--burst <VALUE>` | New burst multiplier |

**Implementation:**
- Read current profile
- Modify specified rate limit value
- Write updated profile back
- Return confirmation with old/new values

---

### Enhancement 2: `config rate-limits enable/disable`

**Purpose:** Quick toggle for rate limiting without editing profile.

**Commands:**
```bash
sp config rate-limits enable
sp config rate-limits disable
```

**Implementation:**
- Read current profile
- Set `rate_limits.disabled` to `false` (enable) or `true` (disable)
- Write updated profile
- Return confirmation

---

### Enhancement 3: `config rate-limits validate`

**Purpose:** Sanity check rate limit configuration.

**Command:**
```bash
sp config rate-limits validate
sp --json config rate-limits validate
```

**Validation Rules:**
1. No zero or negative base rates
2. Tier multipliers are positive
3. Multiplier hierarchy: `anon < user < admin`
4. Burst multiplier is reasonable (1-10x)
5. No excessive rates that could cause resource exhaustion

**Output:**
```json
{
  "valid": true,
  "errors": [],
  "warnings": [
    "Burst multiplier 15 exceeds recommended maximum of 10"
  ]
}
```

---

### Enhancement 4: `config rate-limits compare`

**Purpose:** Side-by-side comparison of effective limits across tiers.

**Commands:**
```bash
sp config rate-limits compare
sp config rate-limits compare --tiers admin,user,anon
sp --json config rate-limits compare
```

**Output Structure:**
```json
{
  "endpoints": [
    {
      "endpoint": "Contexts",
      "tiers": {
        "admin": 500,
        "user": 50,
        "a2a": 250,
        "mcp": 250,
        "service": 250,
        "anon": 25
      }
    }
  ]
}
```

---

### Enhancement 5: `config rate-limits reset`

**Purpose:** Reset rate limits to default values.

**Commands:**
```bash
# Reset all rate limits to defaults
sp config rate-limits reset --yes

# Reset specific endpoint
sp config rate-limits reset --endpoint contexts --yes

# Reset specific tier multiplier
sp config rate-limits reset --tier admin --yes

# Preview without applying
sp config rate-limits reset --dry-run
```

**Required Flags:**
| Flag | Description |
|------|-------------|
| `--yes` / `-y` | Required for destructive operation |
| `--dry-run` | Preview changes without applying |
| `--endpoint <NAME>` | Reset specific endpoint only |
| `--tier <NAME>` | Reset specific tier multiplier only |

---

## Implementation Plan

### Phase 1: Core Infrastructure
1. Add profile write capability to support modifications
2. Create default rate limits constants

### Phase 2: Basic Commands
1. Implement `enable` command
2. Implement `disable` command
3. Implement `validate` command

### Phase 3: Modification Commands
1. Implement `set` command with endpoint/tier/burst options
2. Implement `reset` command with selective reset

### Phase 4: Comparison Command
1. Implement `compare` command with tier filtering

### Phase 5: Documentation
1. Update README.md with all new commands
2. Add examples for each command

---

## Files to Modify

| File | Changes |
|------|---------|
| `types.rs` | Add output types for new commands |
| `rate_limits.rs` | Add new command implementations |
| `mod.rs` | Register new subcommands |
| `README.md` | Document new commands |

---

## Default Rate Limits

For `reset` command, these are the default values:

```rust
pub const DEFAULT_RATE_LIMITS: RateLimitsConfig = RateLimitsConfig {
    disabled: false,
    oauth_public_per_second: 2,
    oauth_auth_per_second: 2,
    contexts_per_second: 50,
    tasks_per_second: 10,
    artifacts_per_second: 15,
    agent_registry_per_second: 20,
    agents_per_second: 3,
    mcp_registry_per_second: 20,
    mcp_per_second: 100,
    stream_per_second: 1,
    content_per_second: 20,
    burst_multiplier: 2,
    tier_multipliers: TierMultipliers {
        admin: 10.0,
        user: 1.0,
        a2a: 5.0,
        mcp: 5.0,
        service: 5.0,
        anon: 0.5,
    },
};
```
