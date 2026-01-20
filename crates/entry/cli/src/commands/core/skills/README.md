# Skills CLI Commands

This document provides complete documentation for AI agents to use the skills CLI commands. All commands support non-interactive mode for automation.

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
| `skills list` | List configured skills | `Table` | No |
| `skills create` | Create new skill | `Text` | No |
| `skills edit <name>` | Edit skill configuration | `Text` | No |
| `skills delete <name>` | Delete a skill | `Text` | No |
| `skills status` | Show database sync status | `Table` | No (DB only) |
| `skills sync` | Sync skills between disk and database | `Text` | No (DB only) |

---

## Core Commands

### skills list

List all configured skills from disk and database.

```bash
sp skills list
sp --json skills list
sp skills list --source disk
sp skills list --source database
sp skills list --agent primary
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--source` | `all` | Source: `all`, `disk`, `database` |
| `--agent` | None | Filter by agent name |

**Output Structure:**
```json
{
  "skills": [
    {
      "name": "code_review",
      "display_name": "Code Review",
      "description": "Reviews code for quality and best practices",
      "agent": "primary",
      "enabled": true,
      "source": "disk",
      "synced": true
    }
  ],
  "total": 1
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `display_name`, `agent`, `enabled`, `synced`

---

### skills create

Create a new skill configuration.

```bash
sp skills create \
  --name "my_skill" \
  --display-name "My Skill" \
  --description "A custom skill" \
  --agent primary \
  --prompt "You are a helpful assistant that..."

sp skills create \
  --name "code_helper" \
  --display-name "Code Helper" \
  --description "Helps with coding tasks" \
  --agent primary \
  --prompt-file ./prompts/code_helper.txt \
  --enabled
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `--name` | Yes | Skill identifier (lowercase alphanumeric + underscores) |
| `--agent` | Yes | Agent to associate skill with |
| `--prompt` or `--prompt-file` | Yes | Skill prompt (inline or file path) |

**Optional Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--display-name` | Same as name | Human-readable name |
| `--description` | Empty | Skill description |
| `--enabled` | `false` | Enable skill after creation |

**Validation Rules:**
- Name: 3-50 characters, lowercase alphanumeric with underscores only
- Agent must exist in configuration

**Output Structure:**
```json
{
  "name": "my_skill",
  "path": "/var/www/html/tyingshoelaces/services/skills/my_skill.yaml",
  "message": "Skill 'my_skill' created successfully"
}
```

**Artifact Type:** `Text`

---

### skills edit

Edit an existing skill configuration.

```bash
sp skills edit <skill_name> --enable
sp skills edit <skill_name> --disable
sp skills edit <skill_name> --description "Updated description"
sp skills edit <skill_name> --prompt "New prompt text..."
sp skills edit <skill_name> --prompt-file ./prompts/updated.txt
sp skills edit <skill_name> --set display_name="New Display Name"
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Skill name to edit |
| At least one change | Yes | Must specify at least one modification |

**Modification Flags:**
| Flag | Description |
|------|-------------|
| `--enable` | Enable the skill |
| `--disable` | Disable the skill |
| `--description` | Update description |
| `--prompt` | Update prompt text inline |
| `--prompt-file` | Update prompt from file |
| `--set <key=value>` | Set arbitrary config value |

**Supported --set Keys:**
- `display_name`
- `description`
- `enabled` (boolean)
- `agent`

**Output Structure:**
```json
{
  "name": "my_skill",
  "message": "Skill 'my_skill' updated successfully with 2 change(s)",
  "changes": ["enabled: true", "description: Updated description"]
}
```

**Artifact Type:** `Text`

---

### skills delete

Delete a skill configuration.

```bash
sp skills delete <skill_name> --yes
sp skills delete my_skill --yes
```

**Required Flags (non-interactive):**
| Flag | Required | Description |
|------|----------|-------------|
| `<name>` | Yes | Skill name to delete |
| `--yes` / `-y` | Yes | Skip confirmation (REQUIRED in non-interactive mode) |

**Output Structure:**
```json
{
  "deleted": "my_skill",
  "message": "Skill 'my_skill' deleted successfully"
}
```

**Artifact Type:** `Text`

---

### skills status

Show database sync status for skills.

```bash
sp skills status
sp --json skills status
sp skills status --agent primary
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--agent` | None | Filter by agent name |

**Output Structure:**
```json
{
  "skills": [
    {
      "name": "code_review",
      "disk_exists": true,
      "db_exists": true,
      "synced": true,
      "disk_updated": "2024-01-15T10:30:00Z",
      "db_updated": "2024-01-15T10:30:00Z"
    },
    {
      "name": "old_skill",
      "disk_exists": false,
      "db_exists": true,
      "synced": false,
      "disk_updated": null,
      "db_updated": "2024-01-01T00:00:00Z"
    }
  ],
  "summary": {
    "total_disk": 5,
    "total_db": 6,
    "synced": 5,
    "unsynced": 1
  }
}
```

**Artifact Type:** `Table`
**Columns:** `name`, `disk_exists`, `db_exists`, `synced`

---

### skills sync

Sync skills between disk and database.

```bash
sp skills sync
sp skills sync --direction to-db
sp skills sync --direction from-db
sp skills sync --agent primary
sp skills sync --dry-run
```

**Flags:**
| Flag | Default | Description |
|------|---------|-------------|
| `--direction` | `to-db` | Sync direction: `to-db`, `from-db` |
| `--agent` | None | Filter by agent name |
| `--dry-run` | `false` | Show what would be synced without making changes |

**Sync Directions:**
- `to-db`: Push disk configurations to database
- `from-db`: Pull database configurations to disk

**Output Structure:**
```json
{
  "direction": "to-db",
  "created": ["new_skill"],
  "updated": ["existing_skill"],
  "deleted": [],
  "skipped": [],
  "dry_run": false,
  "message": "Synced 2 skill(s) to database"
}
```

**Artifact Type:** `Text`

---

## Complete Skills Management Flow Example

This flow demonstrates the full skills lifecycle:

```bash
# Phase 1: List existing skills
sp --json skills list
sp --json skills status

# Phase 2: Create new skill
sp skills create \
  --name "documentation_helper" \
  --display-name "Documentation Helper" \
  --description "Helps write and improve documentation" \
  --agent primary \
  --prompt "You are a technical writer assistant. Help users create clear, comprehensive documentation."

# Phase 3: Verify creation
sp --json skills list --source disk
sp --json skills status

# Phase 4: Sync to database
sp skills sync --direction to-db

# Phase 5: Enable skill
sp skills edit documentation_helper --enable

# Phase 6: Update skill
sp skills edit documentation_helper \
  --description "Professional documentation assistance" \
  --prompt "You are an expert technical writer..."

# Phase 7: Re-sync after update
sp skills sync --direction to-db

# Phase 8: Delete skill
sp skills delete documentation_helper --yes

# Phase 9: Sync deletion
sp skills sync --direction to-db

# Phase 10: Verify deletion
sp --json skills list
```

---

## Skill Configuration File Format

Skills are stored as YAML files:

```yaml
# /services/skills/my_skill.yaml
name: my_skill
display_name: My Skill
description: A helpful skill
agent: primary
enabled: true
prompt: |
  You are a helpful assistant that specializes in...

  When responding:
  1. Be clear and concise
  2. Provide examples when helpful
  3. Ask clarifying questions if needed
```

---

## Prompt File Format

For complex prompts, use a separate file:

```bash
# Create prompt file
cat << 'EOF' > ./prompts/my_skill.txt
You are a helpful assistant that specializes in software development.

When responding to requests:
1. Analyze the problem carefully
2. Provide clear, actionable solutions
3. Include code examples when appropriate
4. Explain your reasoning

Always follow best practices and consider edge cases.
EOF

# Create skill with prompt file
sp skills create \
  --name "dev_helper" \
  --agent primary \
  --prompt-file ./prompts/my_skill.txt
```

---

## Error Handling

### Missing Required Flags

```bash
sp skills create --name test
# Error: --agent is required in non-interactive mode

sp skills create --name test --agent primary
# Error: --prompt or --prompt-file is required in non-interactive mode

sp skills delete my_skill
# Error: --yes is required to delete skills in non-interactive mode
```

### Validation Errors

```bash
sp skills create --name "Test Skill" --agent primary --prompt "test"
# Error: Skill name must be lowercase alphanumeric with underscores only

sp skills create --name "ab" --agent primary --prompt "test"
# Error: Skill name must be between 3 and 50 characters

sp skills create --name "new_skill" --agent nonexistent --prompt "test"
# Error: Agent 'nonexistent' not found
```

### Not Found Errors

```bash
sp skills edit nonexistent --enable
# Error: Skill 'nonexistent' not found

sp skills delete nonexistent --yes
# Error: Skill 'nonexistent' not found
```

### Sync Errors

```bash
sp skills sync
# Error: Failed to connect to database. Check your profile configuration.
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json skills list | jq .

# Extract specific fields
sp --json skills list | jq '.skills[].name'
sp --json skills status | jq '.summary'
sp --json skills list | jq '.skills[] | select(.enabled == true)'
sp --json skills status | jq '.skills[] | select(.synced == false)'
```

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] `delete` command requires `--yes` / `-y` flag
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `render_result()`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] Proper error messages for missing required flags
