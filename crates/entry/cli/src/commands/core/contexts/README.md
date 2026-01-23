<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://docs.systemprompt.io">Documentation</a></p>
</div>

---


# Contexts CLI Commands

Manage CLI conversation contexts. Each context maintains separate conversation history and state.

---

## Prerequisites

```bash
export SYSTEMPROMPT_PROFILE=/path/to/profile.yaml
cd /var/www/html/systemprompt-core
cargo build --package systemprompt-cli

alias sp="./target/debug/systemprompt --non-interactive"
```

---

## Command Reference

| Command | Description | Artifact Type |
|---------|-------------|---------------|
| `core contexts list` | List all contexts with stats | `Table` |
| `core contexts show <id\|name>` | Display context details | `Card` |
| `core contexts create` | Create new context | `Card` |
| `core contexts edit <id\|name>` | Rename a context | `Card` |
| `core contexts delete <id\|name>` | Delete a context | `Card` |
| `core contexts use <id\|name>` | Switch active context | `Card` |
| `core contexts new` | Create and switch (shortcut) | `Card` |

---

## Context Resolution

All commands that accept `<id|name>` support flexible resolution:

| Input Type | Example | Description |
|------------|---------|-------------|
| Full UUID | `a1b2c3d4-e5f6-7890-abcd-ef1234567890` | Exact ID match |
| Partial ID | `a1b2c3d4` | Prefix match (min 4 chars) |
| Name | `My Project` | Exact or case-insensitive name match |

---

## Commands

### contexts list

List all contexts with statistics.

```bash
sp core contexts list
sp --json contexts list
```

**Output columns:** ID (truncated), Name, Tasks, Messages, Updated, Active

**JSON output:**
```json
{
  "contexts": [
    {
      "id": "a1b2c3d4-...",
      "name": "CLI Session - local",
      "task_count": 5,
      "message_count": 23,
      "created_at": "2024-01-15T10:00:00Z",
      "updated_at": "2024-01-15T12:30:00Z",
      "last_message_at": "2024-01-15T12:30:00Z",
      "is_active": true
    }
  ],
  "total": 1,
  "active_context_id": "a1b2c3d4-..."
}
```

---

### contexts show

Display detailed information about a context.

```bash
sp core contexts show a1b2c3d4
sp core contexts show "My Project"
sp --json contexts show a1b2c3d4
```

**Arguments:**
- `<context>` - Context ID (full or partial) or name

---

### contexts create

Create a new context without switching to it.

```bash
sp core contexts create
sp core contexts create --name "My Project"
sp --json contexts create --name "API Testing"
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--name <NAME>` | Name for the context (default: auto-generated with timestamp) |

---

### contexts edit

Rename an existing context.

```bash
sp core contexts edit a1b2c3d4 --name "New Name"
sp core contexts edit "Old Name" --name "New Name"
```

**Arguments:**
- `<context>` - Context ID (full or partial) or name

**Flags:**
| Flag | Required | Description |
|------|----------|-------------|
| `--name <NAME>` | Yes | New name for the context |

---

### contexts delete

Delete a context. Cannot delete the active context.

```bash
sp core contexts delete a1b2c3d4 --yes
sp core contexts delete "Old Project" -y
```

**Arguments:**
- `<context>` - Context ID (full or partial) or name

**Flags:**
| Flag | Description |
|------|-------------|
| `-y, --yes` | Skip confirmation prompt |

**Note:** You cannot delete the currently active context. Switch to a different context first using `core contexts use`.

---

### contexts use

Switch the session's active context.

```bash
sp core contexts use a1b2c3d4
sp core contexts use "My Project"
```

**Arguments:**
- `<context>` - Context ID (full or partial) or name

This updates the session file so subsequent `admin agents message` commands use the selected context.

---

### contexts new

Create a new context and immediately switch to it (shortcut for `create` + `use`).

```bash
sp core contexts new
sp core contexts new --name "New Session"
sp --json contexts new --name "Debug Session"
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--name <NAME>` | Name for the context (default: auto-generated with timestamp) |

---

## Workflows

### Starting a fresh conversation

```bash
# Create new context and switch to it
sp core contexts new --name "Bug Investigation"

# Send messages in the new context
sp admin agents message primary -m "Help me debug this issue"
```

### Switching between projects

```bash
# List available contexts
sp core contexts list

# Switch to a different context
sp core contexts use "Project A"

# Continue conversation in that context
sp admin agents message primary -m "Continue from where we left off"
```

### Cleaning up old contexts

```bash
# List contexts to find old ones
sp --json contexts list | jq '.contexts[] | select(.message_count == 0)'

# Delete unused context
sp core contexts delete "Old Test" --yes
```
