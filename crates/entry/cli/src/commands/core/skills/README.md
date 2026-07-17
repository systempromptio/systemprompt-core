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


# Skills CLI Commands

Inspect the skills configured on infrastructure you own. Skills are read from the profile's skills directory, so what the CLI reports is what the agents actually load. Both commands run non-interactively for automation.

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

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>CLI reference · Own how your organization uses AI.</sub>

</div>
