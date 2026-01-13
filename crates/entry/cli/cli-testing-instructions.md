# CLI Testing Instructions

## Overview

This is the **CORE** repository (`systemprompt-core`). All CLI development and changes should be made here.

For testing, use the local profile from the `tyingshoelaces` project which provides a complete services configuration.

## Repositories

| Repository | Path | Purpose |
|------------|------|---------|
| **systemprompt-core** | `/var/www/html/systemprompt-core` | Core library and CLI source code (make changes here) |
| tyingshoelaces | `/var/www/html/tyingshoelaces` | Test environment with services and local profile |

## Local Profile

```
/var/www/html/tyingshoelaces/.systemprompt/profiles/local/profile.yaml
```

This profile provides database connection and services configuration needed to test most CLI commands.

## Testing Workflow

### 1. Build Debug

From the core repository:

```bash
cd /var/www/html/systemprompt-core
cargo build --package systemprompt-cli
```

The debug binary will be at: `target/debug/systemprompt`

### 2. Test CLI Commands

Set the profile and run commands:

```bash
export SYSTEMPROMPT_PROFILE=/var/www/html/tyingshoelaces/.systemprompt/profiles/local/profile.yaml

# Test in non-interactive mode (recommended for validation)
./target/debug/systemprompt --non-interactive <command>

# Examples
./target/debug/systemprompt --non-interactive logs stream view --limit 5
./target/debug/systemprompt --non-interactive agents list
./target/debug/systemprompt --non-interactive services db status
```

### 3. Running Services (When Required)

**Most commands only need the database connection** - the local profile provides this.

For commands that require running services (e.g., sending messages to agents, real-time streaming), start services in the tyingshoelaces repo:

```bash
cd /var/www/html/tyingshoelaces
just start
```

Commands that typically require running services:
- `logs trace view` (when sending a new message with `-m`)
- `logs trace ai` (when tracing live agent execution)
- Any command that interacts with running agents

Commands that only need database:
- `logs stream view/delete/cleanup`
- `logs trace list/lookup`
- `agents list/show/validate`
- `services db *`
- Most read-only operations

## Test Results

Test results are stored in markdown files under:
```
/var/www/html/systemprompt-core/crates/entry/cli/test/
```

Each file documents:
- Command tested
- Pass/Fail status
- Output captured

## Quick Reference

```bash
# One-liner to build and test a command
cd /var/www/html/systemprompt-core && \
cargo build --package systemprompt-cli && \
SYSTEMPROMPT_PROFILE=/var/www/html/tyingshoelaces/.systemprompt/profiles/local/profile.yaml \
./target/debug/systemprompt --non-interactive <command>
```
