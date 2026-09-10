# Skills CLI Commands

Command reference for skills. Use the installed command’s `--help` output for its complete arguments and defaults.

---

## Prerequisites

```bash
export SYSTEMPROMPT_PROFILE=~/.systemprompt/profiles/local/profile.yaml
cd /var/www/html/systemprompt-core
cargo build --package systemprompt-cli

alias sp="./target/debug/systemprompt --non-interactive"
```

---

## Command Reference

| Command | Description | Artifact Type | Requires Services |
|---------|-------------|---------------|-------------------|
| `core skills list` | List configured skills | `Table` | No |
| `core skills show <id>` | Show skill details | `Card` | No |

---

## Commands

### skills list

List skills discovered in the profile's skills directory. Passing a skill ID as the positional argument renders that single skill's detail card instead of the table.

```bash
sp core skills list
sp --json core skills list
sp core skills list --enabled
sp core skills list --disabled
sp core skills list code_review
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | No | Skill ID; when given, shows that skill's details |

**Flags:**
| Flag | Description |
|------|-------------|
| `--enabled` | Show only enabled skills (conflicts with `--disabled`) |
| `--disabled` | Show only disabled skills (conflicts with `--enabled`) |

**Output Structure:**
```json
{
  "skills": [
    {
      "skill_id": "code_review",
      "name": "code_review",
      "display_name": "Code Review",
      "enabled": true,
      "file_path": "/services/skills/code_review/SKILL.md",
      "tags": ["review", "quality"]
    }
  ]
}
```

When a skill ID is passed, the command instead returns a detail card with
`skill_id`, `name`, `display_name`, `description`, `enabled`, `tags`,
`category`, `file_path`, and `instructions_preview`.

**Artifact Type:** `Table`

---

### skills show

Show details for a single skill by its ID (directory name), including an instructions preview.

```bash
sp core skills show code_review
sp --json core skills show code_review
```

**Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<name>` | Yes | Skill ID (directory name) |

**Artifact Type:** `Card`

---

## JSON Output

Both commands support `--json` for structured output:

```bash
sp --json core skills list | jq '.skills[].name'
sp --json core skills show code_review | jq '.instructions_preview'

sp --json core skills list | jq '.skills[] | select(.enabled == true)'
```

---
