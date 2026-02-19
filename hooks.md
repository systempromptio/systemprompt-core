# Hook Catalog Migration: Core Integration Report

## What Was Added in Extensions

The marketplace project (`systemprompt-claude-marketplace`) has migrated hooks from an embedded-in-plugin-config approach to first-class entities with their own folder structure, database schema, and catalog repository. This mirrors how skills already work.

### New On-Disk Structure

```
services/hooks/
  tracking_post_tool_use/
    config.yaml              # Hook metadata and behavior
    script.sh.tmpl           # Bash tracking template
    script.ps1.tmpl          # PowerShell tracking template
  tracking_post_tool_use_failure/
  tracking_session_start/
  tracking_session_end/
  tracking_user_prompt_submit/
  tracking_stop/
  tracking_subagent_start/
  tracking_subagent_stop/
```

Each `config.yaml` follows this schema:
```yaml
id: tracking_session_start      # Stable ID (matches directory name)
name: "Session Start Tracking"
description: "Records when user sessions begin"
version: "1.0.0"
enabled: true
event: SessionStart             # Must match a valid HookEvent
matcher: "*"
command: "${CLAUDE_PLUGIN_ROOT}/scripts/track-{plugin_id}-usage.sh"
async: true
category: system                # "system" or "custom"
tags: [tracking, analytics]
visible_to: [admin]
```

### New Database Tables

- `hook_catalog` — mirrors `agent_skills` pattern; stores hook definitions with SHA256 checksums
- `hook_plugins` — N:M join table associating hooks with plugins
- `hook_files` — mirrors `skill_files`; stores file inventory per hook directory

### New Repository: `hook_catalog.rs`

- `list_file_hooks()` — scan `services/hooks/*/config.yaml`
- `sync_hooks()` — checksum-based upsert to `hook_catalog` table + auto-associates system hooks with all plugins via `hook_plugins`
- `sync_hook_files()` — sync all files in hook directories to `hook_files` table
- `list_catalog_hooks()` / `get_catalog_hook()` — DB queries with plugin enrichment
- `create_catalog_hook()` / `update_catalog_hook()` / `delete_catalog_hook()` — CRUD (writes to disk + DB)
- `catalog_to_detail()` — converts `HookCatalogEntry` to legacy `HookDetail` for API compat
- `build_hooks_json_from_catalog()` — generates export JSON from catalog entries
- `render_tracking_script()` — renders `script.sh.tmpl` with `{{plugin_id}}`, `{{token}}`, `{{platform_url}}`
- `read_hook_template()` — reads script templates from hook directories on disk

### What Changed in Handlers / UI

- API handlers (`hooks.rs`) now use catalog CRUD instead of editing plugin `config.yaml` directly
- SSR hooks page now shows real hooks from the catalog (8 system tracking hooks), not phantom hooks generated per-plugin
- Old `hooks.rs` repository kept as legacy fallback (read-only: `list_hooks`, `get_hook`)

### Export Pipeline Changes

- `build_tracking_script_from_template()` — reads `script.sh.tmpl` from disk, renders with `{{plugin_id}}`, `{{token}}`, `{{platform_url}}`. Falls back to hardcoded template if not found.
- `build_tracking_script_ps1_from_template()` — same for PowerShell
- `build_tracking_hooks_from_catalog()` — builds hooks JSON from catalog entries on disk instead of from `TRACKING_EVENTS` constant. Falls back to constant-based generation if catalog is empty.
- Export output format is unchanged — Claude Code compatibility preserved.

### Plugin-Hook Association

- System hooks (category=system) are auto-associated with ALL plugins during `sync_hooks()`
- Custom hooks are associated with specific plugins via the `hook_plugins` DB table during creation
- No changes to plugin `config.yaml` format needed — core's `PluginConfig.hooks` field remains `HookEventsConfig` for backward compatibility
- The association is managed through the DB (`hook_plugins` table), not through plugin config files

---

## What Core Currently Has

### `crates/shared/models/src/services/hooks.rs`

```rust
pub struct HookEventsConfig {           // Embedded in PluginConfig.hooks
    pub pre_tool_use: Vec<HookMatcher>,
    pub post_tool_use: Vec<HookMatcher>,
    pub session_start: Vec<HookMatcher>,
    // ... 9 event fields total
}

pub struct HookMatcher {
    pub matcher: String,
    pub hooks: Vec<HookAction>,
}

pub struct HookAction {
    pub hook_type: HookType,            // Command | Prompt | Agent
    pub command: Option<String>,
    pub prompt: Option<String>,
    pub r#async: bool,
    pub timeout: Option<u32>,
    pub status_message: Option<String>,
}
```

### `crates/shared/models/src/services/plugin.rs`

```rust
pub struct PluginConfig {
    pub hooks: HookEventsConfig,        // Inline hook definitions
    pub skills: PluginComponentRef,     // Reference-based: include: [skill_id, ...]
    // ...
}
```

### Current Core Skills Pattern (Reference Architecture)

```rust
pub struct PluginComponentRef {         // How plugins reference skills
    pub source: ComponentSource,        // Instance | Explicit
    pub include: Vec<String>,           // ["skill_a", "skill_b"]
    pub exclude: Vec<String>,
}

pub struct DiskSkillConfig {            // On-disk config.yaml schema
    pub id: String,
    pub name: String,
    pub description: String,
    pub enabled: bool,
    pub file: String,                   // Content file (default: "index.md")
    pub tags: Vec<String>,
    pub category: Option<String>,
}
```

---

## What Core Needs for Full Integration

### 1. `DiskHookConfig` — On-Disk Hook Schema (Parallel to `DiskSkillConfig`)

Core needs a type that parses `services/hooks/{id}/config.yaml`:

```rust
// New: crates/shared/models/src/services/hooks.rs

pub const HOOK_CONFIG_FILENAME: &str = "config.yaml";

#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DiskHookConfig {
    pub id: String,
    pub name: String,
    #[serde(default)]
    pub description: String,
    #[serde(default = "default_version")]
    pub version: String,
    #[serde(default = "default_true")]
    pub enabled: bool,
    pub event: HookEvent,               // Strongly typed enum, not String
    #[serde(default = "default_matcher")]
    pub matcher: String,
    #[serde(default)]
    pub command: String,
    #[serde(default, rename = "async")]
    pub is_async: bool,
    #[serde(default)]
    pub category: HookCategory,         // Strongly typed enum
    #[serde(default)]
    pub tags: Vec<String>,
    #[serde(default)]
    pub visible_to: Vec<String>,
}
```

Currently, extensions define their own `HookConfig` struct that parses the same file but with `String` fields instead of enums. Core should own this type for strong validation.

### 2. `HookEvent` Enum — Strongly Typed Event Names

Currently, event names are strings everywhere. Core should define them as an enum:

```rust
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize, JsonSchema)]
pub enum HookEvent {
    PreToolUse,
    PostToolUse,
    PostToolUseFailure,
    SessionStart,
    SessionEnd,
    UserPromptSubmit,
    Notification,
    Stop,
    SubagentStart,
    SubagentStop,
}
```

This replaces:
- The `TRACKING_EVENTS: &[&str]` constant in extensions
- The hardcoded event list in the SSR hook edit page
- The `HookEventsConfig` struct field names (which are the PascalCase version of these)

### 3. `HookCategory` Enum

```rust
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "lowercase")]
pub enum HookCategory {
    System,
    #[default]
    Custom,
}
```

### 4. `PluginConfig.hooks` — Dual Format Support (Future)

Currently, `PluginConfig.hooks` is `HookEventsConfig` (inline definitions). Long-term it should support BOTH inline definitions AND catalog references:

```rust
pub struct PluginConfig {
    // Option A: Keep both (backward compat)
    #[serde(default)]
    pub hooks: HookEventsConfig,        // Inline hook definitions (existing)
    #[serde(default)]
    pub hook_refs: PluginComponentRef,  // Catalog references (new)
    // hook_refs:
    //   source: explicit
    //   include:
    //     - tracking_session_start
    //     - my_custom_hook
}
```

**Current state**: No plugins have inline hooks in `config.yaml`. System hooks are associated via the `hook_plugins` DB table (populated by `sync_hooks()`). Custom hooks are created via the admin UI. So this change is **not blocking** — the extension handles it entirely through the DB.

**When to implement**: When a plugin needs to declare custom hook dependencies in its config file (e.g., for portable plugin bundles that need specific hooks).

### 5. Hook Validation in Core

Add validation for `DiskHookConfig`:

```rust
impl DiskHookConfig {
    pub fn validate(&self, dir_name: &str) -> anyhow::Result<()> {
        if self.id != dir_name {
            anyhow::bail!("Hook ID '{}' does not match directory name '{}'", self.id, dir_name);
        }
        if self.name.is_empty() {
            anyhow::bail!("Hook '{}': name must not be empty", self.id);
        }
        if self.command.is_empty() && self.category == HookCategory::Custom {
            anyhow::bail!("Hook '{}': custom hooks require a command", self.id);
        }
        Ok(())
    }
}
```

### 6. Hook Discovery / Loading in Core Loader

The `systemprompt_loader` crate should discover hooks from `services/hooks/` alongside skills from `services/skills/`. If it already has a `load_skills()` function that walks `services/skills/*/config.yaml`, a parallel `load_hooks()` should walk `services/hooks/*/config.yaml`.

### 7. Exports from `services/mod.rs`

Add to the public API surface:

```rust
pub use hooks::{
    DiskHookConfig, HookAction, HookCategory, HookEvent, HookEventsConfig, HookMatcher,
    HookType, HOOK_CONFIG_FILENAME,
};
```

---

## Boundary Between Core and Extensions

| Concern | Core | Extensions |
|---------|------|------------|
| **Type definitions** | `DiskHookConfig`, `HookEvent`, `HookCategory`, `HookEventsConfig` | `HookCatalogEntry` (DB model), `HookDetail` (API model) |
| **On-disk parsing** | Deserialize `config.yaml` with strong types | N/A (use core types) |
| **Validation** | `DiskHookConfig::validate()`, `HookEventsConfig::validate()` | Business rules (RBAC, system hook protection) |
| **Discovery** | Walk `services/hooks/` dir, parse configs | N/A (call core loader) |
| **DB sync** | N/A | `hook_catalog.rs`: sync disk -> `hook_catalog` table |
| **CRUD** | N/A | `hook_catalog.rs`: create/update/delete |
| **Export** | `HookEventsConfig` as output format | `build_hooks_json_from_catalog()`, script template rendering |
| **Admin UI** | N/A | SSR pages, API handlers |
| **CLI** | `systemprompt core hooks` subcommand structure | Hook management commands |

### What Must NOT Be in Extensions

- Hook event names (should come from `HookEvent` enum in core)
- Hook config parsing logic (should use `DiskHookConfig` from core)
- The `TRACKING_EVENTS` constant (should derive from `HookEvent::iter()` or similar)

### What Must NOT Be in Core

- Database tables and queries
- HTTP handlers and SSR templates
- Activity logging
- Plugin export bundle generation
- SHA256 sync logic

---

## Migration Steps for Core

1. **Add `DiskHookConfig`** to `crates/shared/models/src/services/hooks.rs`
2. **Add `HookEvent` enum** with all 10 event types
3. **Add `HookCategory` enum** (system/custom)
4. **Add `HOOK_CONFIG_FILENAME`** constant
5. **Add validation** methods to `DiskHookConfig`
6. **Update exports** in `services/mod.rs`
7. **Add hook loading** to the loader crate (parallel to skills loading)
8. **Update `PluginConfig.hooks`** from `HookEventsConfig` to `PluginComponentRef` (breaking change — needs coordinated migration)
9. **Bump version** to 0.1.14 (or 0.2.0 if `PluginConfig.hooks` change is breaking)

### Step 8 Coordination

The `PluginConfig.hooks` change from `HookEventsConfig` to `PluginComponentRef` is breaking. To migrate safely:

1. First, support **both** formats via an enum or `#[serde(untagged)]`:
   ```rust
   #[serde(default)]
   pub hooks: HookRef,  // Accepts either HookEventsConfig or PluginComponentRef
   ```
2. Extensions migrate to writing `hooks.include` in plugin configs
3. Old inline `HookEventsConfig` format deprecated
4. Eventually remove inline support

---

## Currently, Extensions Work Around Core's Gaps

The extensions crate defines its own `HookConfig` struct in `hook_catalog.rs` with `String` fields where core should provide typed enums. Once core adds `DiskHookConfig` and `HookEvent`, extensions should:

1. Replace the local `HookConfig` struct with `systemprompt::models::DiskHookConfig`
2. Replace `String` event fields with `systemprompt::models::HookEvent`
3. Use core's validation instead of local checks
4. Replace `TRACKING_EVENTS` constant with core's `HookEvent` enum variants
