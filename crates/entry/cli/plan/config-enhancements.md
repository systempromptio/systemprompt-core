# Config Domain Enhancements

## Overview

This document tracks the comprehensive enhancements to the `config` CLI domain, expanding from rate-limits only to full configuration management.

---

## New Subcommands

### 1. `config show` - Configuration Overview
Shows all configuration sections in one view.

```bash
sp config show
sp config show --section server
sp config show --section runtime
sp --json config show
```

### 2. `config server` - Server Configuration
```bash
sp config server show
sp config server set --host 0.0.0.0
sp config server set --port 8080
sp config server set --use-https true
sp config server cors add https://example.com
sp config server cors remove https://example.com
sp config server cors list
```

### 3. `config runtime` - Runtime Configuration
```bash
sp config runtime show
sp config runtime set --environment production
sp config runtime set --log-level debug
sp config runtime set --output-format json
sp config runtime set --no-color true
```

### 4. `config security` - Security Configuration
```bash
sp config security show
sp config security set --jwt-expiry 3600
sp config security set --refresh-expiry 86400
```

### 5. `config paths` - Paths Configuration
```bash
sp config paths show
sp config paths validate
```

---

## Rate-Limits Enhancements

### Presets
```bash
sp config rate-limits preset list
sp config rate-limits preset show development
sp config rate-limits preset apply production --yes
sp config rate-limits preset save my-preset --yes
```

### Import/Export
```bash
sp config rate-limits export --output limits.yaml
sp config rate-limits export --format json --output limits.json
sp config rate-limits import --file limits.yaml --yes
```

### Diff
```bash
sp config rate-limits diff --defaults
sp config rate-limits diff --file other-limits.yaml
```

---

## Implementation Files

| File | Purpose |
|------|---------|
| `mod.rs` | Route all subcommands |
| `types.rs` | All output types |
| `show.rs` | Overview command |
| `server.rs` | Server config commands |
| `runtime.rs` | Runtime config commands |
| `security.rs` | Security config commands |
| `paths.rs` | Paths config commands |
| `rate_limits.rs` | Enhanced with presets/import/export/diff |

---

## Presets Data

### Development Preset
- High rate limits for testing
- Rate limiting disabled
- All multipliers at 10x

### Production Preset
- Conservative rate limits
- Rate limiting enabled
- Standard tier hierarchy

### High-Traffic Preset
- Doubled base rates
- Higher burst multiplier (5x)
- Rate limiting enabled
