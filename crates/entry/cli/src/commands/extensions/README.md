# Extensions CLI Commands

This document provides complete documentation for AI agents to use the extensions CLI commands. All commands support non-interactive mode for automation.

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
| `extensions list` | List all discovered extensions | `Table` | No |
| `extensions show <id>` | Show detailed extension info | `Card` | No |
| `extensions validate` | Validate extension dependencies | `Card` | No |
| `extensions config <id>` | Show extension configuration | `Card` | No |
| `extensions capabilities jobs` | List all jobs across extensions | `Table` | No |
| `extensions capabilities templates` | List all templates | `Table` | No |
| `extensions capabilities schemas` | List all database schemas | `Table` | No |
| `extensions capabilities tools` | List all tool providers | `Table` | No |
| `extensions capabilities roles` | List all roles | `Table` | No |
| `extensions capabilities llm-providers` | List all LLM providers | `Table` | No |

---

## Core Commands

### extensions list

List all discovered extensions from the registry.

```bash
sp extensions list
sp --json extensions list
sp extensions list --filter blog
sp extensions list --capability jobs
sp extensions list --capability templates
sp extensions list --capability schemas
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--filter <string>` | Filter by extension ID or name (substring match) |
| `--capability <type>` | Filter by capability type |

**Capability Types:** `jobs`, `templates`, `schemas`, `routes`, `tools`, `roles`, `llm`, `storage`

**Output Structure:**
```json
{
  "extensions": [
    {
      "id": "blog",
      "name": "Blog Extension",
      "version": "1.0.0",
      "priority": 100,
      "source": "compiled",
      "enabled": true,
      "capabilities": {
        "jobs": 1,
        "templates": 3,
        "schemas": 7,
        "routes": 0,
        "tools": 0,
        "roles": 2,
        "llm_providers": 0,
        "storage_paths": 2
      }
    }
  ],
  "total": 1
}
```

**Artifact Type:** `Table`
**Columns:** `id`, `name`, `version`, `priority`, `source`, `capabilities`

---

### extensions show

Display detailed information for a specific extension.

```bash
sp extensions show <extension-id>
sp --json extensions show blog
sp --json extensions show core
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | Extension ID to show |

**Output Structure:**
```json
{
  "id": "blog",
  "name": "Blog Extension",
  "version": "1.0.0",
  "priority": 100,
  "source": "compiled",
  "dependencies": [],
  "config_prefix": "blog",
  "jobs": [
    {
      "name": "ContentIngestionJob",
      "schedule": "0 0 * * * *",
      "enabled": true
    }
  ],
  "templates": [
    {
      "name": "blog-post",
      "description": "text/html, text/markdown"
    }
  ],
  "schemas": [
    {
      "table": "markdown_content",
      "source": "schemas/001_markdown_content.sql",
      "required_columns": ["id", "slug", "content"]
    }
  ],
  "routes": [],
  "tools": [],
  "roles": [
    {
      "name": "content_editor",
      "display_name": "Content Editor",
      "description": "Can edit blog content",
      "permissions": ["content:read", "content:write"]
    }
  ],
  "llm_providers": [],
  "storage_paths": ["content", "uploads"]
}
```

**Artifact Type:** `Card`

---

### extensions validate

Check extension configurations for errors and warnings.

```bash
sp extensions validate
sp --json extensions validate
sp extensions validate --verbose
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--verbose` | Show detailed validation information |

**Output Structure:**
```json
{
  "valid": true,
  "extension_count": 3,
  "errors": [],
  "warnings": [
    {
      "extension_id": "blog",
      "warning_type": "config",
      "message": "Config prefix defined but schema is null"
    }
  ]
}
```

**Validation Checks:**
- Missing dependencies between extensions
- Config prefix defined without config schema
- Default migration weight usage (verbose mode)

**Artifact Type:** `Card`

---

### extensions config

Show configuration details for a specific extension.

```bash
sp extensions config <extension-id>
sp --json extensions config blog
```

**Required Arguments:**
| Argument | Required | Description |
|----------|----------|-------------|
| `<id>` | Yes | Extension ID to show config for |

**Output Structure:**
```json
{
  "extension_id": "blog",
  "config_prefix": "blog",
  "config_schema": {
    "type": "object",
    "properties": {
      "content_dir": { "type": "string" }
    }
  },
  "has_config": true
}
```

**Artifact Type:** `Card`

---

## Capabilities Commands

These commands list specific capabilities across all extensions.

### extensions capabilities jobs

List all scheduled jobs from all extensions.

```bash
sp extensions capabilities jobs
sp --json extensions capabilities jobs
sp extensions capabilities jobs --extension blog
sp extensions capabilities jobs --enabled
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--extension <id>` | Filter by extension ID |
| `--enabled` | Show only enabled jobs |

**Output Structure:**
```json
{
  "jobs": [
    {
      "extension_id": "blog",
      "extension_name": "Blog Extension",
      "job_name": "ContentIngestionJob",
      "schedule": "0 0 * * * *",
      "enabled": true
    },
    {
      "extension_id": "core",
      "extension_name": "Core Extension",
      "job_name": "SessionCleanupJob",
      "schedule": "0 */15 * * * *",
      "enabled": true
    }
  ],
  "total": 2
}
```

**Artifact Type:** `Table`
**Columns:** `extension_id`, `job_name`, `schedule`, `enabled`

---

### extensions capabilities templates

List all templates from all extensions.

```bash
sp extensions capabilities templates
sp --json extensions capabilities templates
sp extensions capabilities templates --extension blog
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--extension <id>` | Filter by extension ID |

**Output Structure:**
```json
{
  "templates": [
    {
      "extension_id": "blog",
      "extension_name": "Blog Extension",
      "template_name": "blog-post",
      "description": "text/html, text/markdown"
    },
    {
      "extension_id": "blog",
      "extension_name": "Blog Extension",
      "template_name": "blog-list",
      "description": "text/html"
    }
  ],
  "total": 2
}
```

**Artifact Type:** `Table`
**Columns:** `extension_id`, `template_name`, `description`

---

### extensions capabilities schemas

List all database schemas from all extensions.

```bash
sp extensions capabilities schemas
sp --json extensions capabilities schemas
sp extensions capabilities schemas --extension blog
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--extension <id>` | Filter by extension ID |

**Output Structure:**
```json
{
  "schemas": [
    {
      "extension_id": "core",
      "extension_name": "Core Extension",
      "table": "users",
      "source": "schemas/001_users.sql",
      "migration_weight": 0
    },
    {
      "extension_id": "blog",
      "extension_name": "Blog Extension",
      "table": "markdown_content",
      "source": "schemas/001_markdown_content.sql",
      "migration_weight": 100
    }
  ],
  "total": 2
}
```

**Notes:**
- Schemas are sorted by `migration_weight` (lower runs first)
- `source` shows either "inline" or the file path

**Artifact Type:** `Table`
**Columns:** `extension_id`, `table`, `migration_weight`, `source`

---

### extensions capabilities tools

List all tool providers from all extensions.

```bash
sp extensions capabilities tools
sp --json extensions capabilities tools
sp extensions capabilities tools --extension mcp
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--extension <id>` | Filter by extension ID |

**Output Structure:**
```json
{
  "tools": [
    {
      "extension_id": "mcp",
      "extension_name": "MCP Extension",
      "tool_name": "tool_provider"
    }
  ],
  "total": 1
}
```

**Artifact Type:** `Table`
**Columns:** `extension_id`, `tool_name`

---

### extensions capabilities roles

List all roles from all extensions.

```bash
sp extensions capabilities roles
sp --json extensions capabilities roles
sp extensions capabilities roles --extension blog
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--extension <id>` | Filter by extension ID |

**Output Structure:**
```json
{
  "roles": [
    {
      "extension_id": "core",
      "extension_name": "Core Extension",
      "role_name": "admin",
      "display_name": "Administrator",
      "description": "Full system access",
      "permissions": ["*"]
    },
    {
      "extension_id": "blog",
      "extension_name": "Blog Extension",
      "role_name": "content_editor",
      "display_name": "Content Editor",
      "description": "Can edit blog content",
      "permissions": ["content:read", "content:write"]
    }
  ],
  "total": 2
}
```

**Artifact Type:** `Table`
**Columns:** `extension_id`, `role_name`, `display_name`, `permissions`

---

### extensions capabilities llm-providers

List all LLM providers from all extensions.

```bash
sp extensions capabilities llm-providers
sp --json extensions capabilities llm-providers
sp extensions capabilities llm-providers --extension ai
```

**Flags:**
| Flag | Description |
|------|-------------|
| `--extension <id>` | Filter by extension ID |

**Output Structure:**
```json
{
  "providers": [
    {
      "extension_id": "ai",
      "extension_name": "AI Extension",
      "provider_name": "llm_provider"
    }
  ],
  "total": 1
}
```

**Artifact Type:** `Table`
**Columns:** `extension_id`, `provider_name`

---

## Complete Discovery Flow Example

This flow demonstrates discovering and inspecting extensions:

```bash
# Phase 1: List all extensions
sp --json extensions list

# Phase 2: Filter by capability
sp --json extensions list --capability jobs
sp --json extensions list --capability schemas

# Phase 3: Show specific extension details
sp --json extensions show blog
sp --json extensions show core

# Phase 4: Validate all extensions
sp --json extensions validate
sp --json extensions validate --verbose

# Phase 5: Check extension configuration
sp --json extensions config blog

# Phase 6: List all capabilities across extensions
sp --json extensions capabilities jobs
sp --json extensions capabilities templates
sp --json extensions capabilities schemas
sp --json extensions capabilities tools
sp --json extensions capabilities roles
sp --json extensions capabilities llm-providers

# Phase 7: Filter capabilities by extension
sp --json extensions capabilities jobs --extension blog
sp --json extensions capabilities schemas --extension core
```

---

## Output Type Summary

| Command | Return Type | Artifact Type | Metadata |
|---------|-------------|---------------|----------|
| `list` | `ExtensionListOutput` | `Table` | columns, title |
| `show` | `ExtensionDetailOutput` | `Card` | title |
| `validate` | `ExtensionValidationOutput` | `Card` | title |
| `config` | `ExtensionConfigOutput` | `Card` | title |
| `capabilities jobs` | `JobsListOutput` | `Table` | columns, title |
| `capabilities templates` | `TemplatesListOutput` | `Table` | columns, title |
| `capabilities schemas` | `SchemasListOutput` | `Table` | columns, title |
| `capabilities tools` | `ToolsListOutput` | `Table` | columns, title |
| `capabilities roles` | `RolesListOutput` | `Table` | columns, title |
| `capabilities llm-providers` | `LlmProvidersListOutput` | `Table` | columns, title |

---

## Error Handling

### Not Found Errors

```bash
sp extensions show nonexistent
# Error: Extension 'nonexistent' not found

sp extensions config nonexistent
# Error: Extension 'nonexistent' not found
```

### Validation Errors

```bash
sp extensions validate
# Shows errors for missing dependencies:
# {
#   "valid": false,
#   "errors": [
#     {
#       "extension_id": "blog",
#       "error_type": "missing_dependency",
#       "message": "Missing dependency: core"
#     }
#   ]
# }
```

---

## JSON Output

All commands support `--json` flag for structured output:

```bash
# Verify JSON is valid
sp --json extensions list | jq .

# Extract specific fields
sp --json extensions list | jq '.extensions[].id'
sp --json extensions show blog | jq '.jobs[].name'
sp --json extensions validate | jq '.valid'
sp --json extensions capabilities jobs | jq '.jobs[] | select(.enabled == true)'
sp --json extensions capabilities schemas | jq '.schemas | sort_by(.migration_weight)'

# Count capabilities
sp --json extensions list | jq '.extensions[] | {id, jobs: .capabilities.jobs, schemas: .capabilities.schemas}'
sp --json extensions capabilities jobs | jq '.total'
```

---

## Extension Source Types

Extensions can come from two sources:

| Source | Description |
|--------|-------------|
| `compiled` | Registered via `register_extension!` macro, discovered at link time via inventory |
| `manifest` | Discovered via `manifest.yaml` files in `/extensions/` directory |

Currently all extensions shown are `compiled` (inventory-based). Manifest-based extensions are discovered separately.

---

## Compliance Checklist

- [x] All `execute` functions accept `config: &CliConfig`
- [x] All commands return `CommandResult<T>` with proper artifact type
- [x] All output types derive `Serialize`, `Deserialize`, `JsonSchema`
- [x] No `println!` / `eprintln!` - uses `render_result()`
- [x] No `unwrap()` / `expect()` - uses `?` with `.context()`
- [x] JSON output supported via `--json` flag
- [x] All commands are synchronous (no async overhead)
- [x] Proper error messages for invalid extension IDs
