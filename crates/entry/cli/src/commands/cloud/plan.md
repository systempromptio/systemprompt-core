# Cloud Domain Migration Plan

**Commands:** `systemprompt cloud [auth|profile|tenant|sync|secrets|deploy|status|restart|init|dockerfile]`

## Current State

### Files
- `mod.rs` - Command router
- `auth/` - Authentication subcommands (login, logout, whoami)
- `profile/` - Profile management
- `tenant.rs` - Tenant management
- `tenant_ops/` - Tenant operations helpers
- `sync/` - Sync operations
- `secrets.rs` - Secrets management
- `deploy.rs` - Deployment
- `deploy_config.rs` - Deployment configuration
- `deploy_select.rs` - Profile selection for deploy
- `dockerfile.rs` - Dockerfile generation
- `status.rs` - Cloud status
- `restart.rs` - Tenant restart
- `init.rs` - Project initialization
- `init_templates.rs` - Init templates
- `checkout/` - Checkout flow
- `oauth/` - OAuth templates

### Migration Status: COMPLETE

| File | Violation | Status |
|------|-----------|--------|
| All | Missing `config: &CliConfig` in execute | FIXED |
| `auth/logout.rs` | Missing `--yes` flag | FIXED |
| `profile/delete.rs` | Missing `--yes` flag | FIXED |
| `tenant.rs` | Missing `--yes` on delete | FIXED |
| `restart.rs` | Missing `--yes` flag | FIXED |
| `auth/login.rs` | No interactive-only check | FIXED |
| `profile/create.rs` | No interactive-only check | FIXED |
| `profile/edit.rs` | No interactive-only check | FIXED |
| `tenant_ops/create.rs` | No interactive-only check | FIXED |
| `tenant_ops/crud.rs` | No interactive-only check | FIXED |
| `sync/*.rs` | Missing CliConfig in execute | FIXED |

---

## Migration Target

Location: `src/commands/cloud/` (complete)

---

## Required Flags

| Command | Required Flags |
|---------|---------------|
| `cloud auth logout` | `--yes` / `-y` |
| `cloud profile delete` | `--yes` / `-y` |
| `cloud tenant delete` | `--id`, `--yes` / `-y` |
| `cloud restart` | `--yes` / `-y` |

## Interactive-Only Commands

| Command | Reason | Alternative |
|---------|--------|-------------|
| `cloud auth login` | Browser OAuth | `set-token` or env var |
| `cloud profile create` | Multi-step wizard | All flags required |
| `cloud profile edit` | Multi-step wizard | All flags required |
| `cloud tenant create` | Multi-step wizard | N/A (requires checkout) |
| `cloud tenant edit` | Multi-step wizard | `--id` + `--set` flags |

---

## Implementation Checklist

- [x] Add `config: &CliConfig` to all execute functions
- [x] Add `--yes` flag to destructive operations
- [x] Add interactive-only checks to OAuth/wizard commands
- [x] Remove all inline comments
- [x] Remove all doc comments
- [x] Split files over 300 lines
- [x] Remove forbidden patterns (println!, unwrap, etc.)

## Validation Status: COMPLETE

All validation checks pass:
- All files under 300 lines
- No inline comments
- No doc comments
- No println!, eprintln!, unwrap(), expect(), panic!, dbg!
- All execute functions have CliConfig parameter
- All destructive operations have --yes flag
