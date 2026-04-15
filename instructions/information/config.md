# Configuration System

This document describes how systemprompt.io loads and validates **services configuration**. It covers `ServicesConfig`, the flat-YAML layout of the `services/` directory, the `includes:` mechanism, the `deny_unknown_fields` contract, and the plugin-binding model.

For profile/secret/credential bootstrap, see `instructions/information/architecture.md`.

---

## Overview

The services configuration is a single in-memory struct, `ServicesConfig`, assembled from YAML files by `ConfigLoader` in `systemprompt-loader`. It is the canonical description of everything a running profile exposes: agents, MCP servers, skills, content sources, web routing, plugins, AI providers, and scheduler jobs.

`ServicesConfig` lives in `crates/shared/models/src/services/mod.rs`:

```rust
pub struct ServicesConfig {
    pub agents:       HashMap<String, AgentConfig>,
    pub mcp_servers:  HashMap<String, Deployment>,
    pub settings:     Settings,
    pub scheduler:    Option<SchedulerConfig>,
    pub ai:           AiConfig,
    pub web:          Option<WebConfig>,
    pub plugins:      HashMap<String, PluginConfig>,
    pub skills:       SkillsConfig,
    pub content:      ContentConfig,
}
```

Notes:

- `skills`, `content`, and `web` are **first-class fields** (Phase 2a/2b/2c). There are no side-channel loaders for them.
- `web` is `systemprompt_provider_contracts::WebConfig` — the stub `WebConfig` and `FullWebConfig`/`WebBrandingConfig` types have been deleted.
- `ServicesConfig`, `PartialServicesConfig`, and the loader's internal `PartialServicesRootConfig` all carry `#[serde(deny_unknown_fields)]`. Typos are loud errors.
- The loader is **pure** — loading never mutates files on disk.

---

## The `services/` Directory Layout

The template project (`systemprompt-template`) uses a **flat YAML layout** — one file per resource — with a single root `services.yaml` that pulls everything together via `includes:`. This is the canonical shape; the core engine mirrors it.

```
services/
├── services.yaml               # Root: settings + includes
├── agents/
│   ├── planner.yaml
│   └── reviewer.yaml
├── mcp/
│   ├── filesystem.yaml
│   └── github.yaml
├── skills/
│   ├── code-review.yaml
│   └── triage.yaml
├── content/
│   ├── blog.yaml
│   └── docs.yaml
├── web/
│   └── web.yaml
├── plugins/
│   └── dev-tools.yaml
├── ai/
│   └── providers.yaml
└── scheduler/
    └── jobs.yaml
```

Each file under a resource directory defines exactly the fields it owns — for example, `agents/planner.yaml` contains a single `agents:` map entry for `planner`. Files can be co-located by concern instead of resource type; the flat layout is a convention, not a schema constraint.

### Example root `services.yaml`

```yaml
settings:
  agent_port_range: [7100, 7199]
  mcp_port_range:   [7200, 7299]

includes:
  - agents/planner.yaml
  - agents/reviewer.yaml
  - mcp/filesystem.yaml
  - mcp/github.yaml
  - skills/code-review.yaml
  - content/blog.yaml
  - web/web.yaml
  - plugins/dev-tools.yaml
  - ai/providers.yaml
  - scheduler/jobs.yaml
```

### Example `agents/planner.yaml`

```yaml
agents:
  planner:
    port: 7101
    default: true
    metadata:
      name: Planner
      system_prompt: "!include ../prompts/planner.md"
```

### Example `mcp/filesystem.yaml`

```yaml
mcp_servers:
  filesystem:
    server_type: stdio
    port: 7201
    command: "npx"
    args: ["-y", "@modelcontextprotocol/server-filesystem", "/workspace"]
```

### Example `skills/code-review.yaml`

```yaml
skills:
  skills:
    code-review:
      id: code-review
      name: Code Review
      assigned_agents: [reviewer]
      mcp_servers:     [github]
```

### Example `content/blog.yaml`

```yaml
content:
  sources:
    blog:
      kind: markdown
      path: ./content/blog
```

---

## `includes:` — Recursive Resolution and Cycle Detection

`ConfigLoader` resolves `includes:` **recursively** (Phase 3a). Each included file is itself a `PartialServicesFile` and may declare its own `includes:`.

Rules:

1. **Paths are relative to the referring file**, not to the loader base. This means an `includes:` inside `agents/team.yaml` that says `./planner.yaml` resolves to `agents/planner.yaml`.
2. **Cycle detection** uses a `visited: HashSet<PathBuf>` of canonicalized absolute paths. If the same file is reached twice, the loader aborts with `Include cycle detected: a -> b -> c -> a` showing the full chain.
3. **Missing includes are hard errors.** The error names both the missing path and the file that referenced it.
4. **Duplicate definitions are hard errors.** Defining the same agent, MCP server, plugin, skill, or content source twice across any combination of files fails the load.
5. The root file's own path is seeded into `visited` before descent, so a root that includes itself is caught.

All include-related errors attribute the *referring* file so users can fix the right YAML.

### `!include` inside strings

`AgentMetadataConfig::system_prompt` and other `IncludableString` fields accept the inline form:

```yaml
metadata:
  system_prompt: "!include ../prompts/planner.md"
```

These are resolved against `base_path` (the directory of the root config file), not the referring include file. This is intentional — prompt files live next to the root, not next to per-resource YAML.

---

## `deny_unknown_fields` — Typos are Loud

`ServicesConfig`, `PartialServicesConfig`, and `PartialServicesRootConfig` all use `#[serde(deny_unknown_fields)]` (Phase 2e). A misspelled key like `mcp_server:` (singular) or `skils:` fails the load with a parse error pointing at the offending file.

This is a deliberate trade-off: the previous behaviour silently dropped unknown keys, which hid real bugs (e.g. a plugin binding under the wrong parent). There is no opt-out. Rename fields carefully and migrate YAML in lock-step with model changes.

---

## Plugins as Binding Descriptors

A plugin YAML is a **binding descriptor**, not a definition store. It names top-level resources that must already exist elsewhere in the merged `ServicesConfig`. `ServicesConfig::validate()` enforces that every reference resolves.

`PluginConfig` (see `crates/shared/models/src/services/plugin.rs`):

```rust
pub struct PluginConfig {
    pub id: String,
    pub name: String,
    pub description: String,
    pub version: String,
    pub enabled: bool,
    pub author: PluginAuthor,
    pub keywords: Vec<String>,
    pub license: String,
    pub category: String,

    pub skills:         PluginComponentRef,  // has `include: Vec<String>`
    pub agents:         PluginComponentRef,  // has `include: Vec<String>`
    pub mcp_servers:    Vec<String>,
    pub content_sources: Vec<String>,
    pub hooks:          HookEventsConfig,
    pub scripts:        Vec<PluginScript>,
}
```

### Example `plugins/dev-tools.yaml`

```yaml
plugins:
  dev-tools:
    id: dev-tools
    name: Developer Tools
    description: Review, plan, and triage bundle
    version: 0.1.0
    enabled: true
    author:
      name: Ed Burton
      email: ed@tyingshoelaces.com
    keywords: [dev, review]
    license: FSL-1.1-ALv2
    category: developer

    agents:
      include: [planner, reviewer]

    skills:
      include: [code-review, triage]

    mcp_servers: [filesystem, github]
    content_sources: [blog]
```

### Validation contract (Phase 2d)

`ServicesConfig::validate()` enforces, for every plugin:

- Each name in `agents.include` must exist in top-level `agents:`.
- Each name in `mcp_servers` must exist in top-level `mcp_servers:`.
- Each name in `skills.include` must exist in top-level `skills.skills:` (warned today, enforced where strict).
- Each name in `content_sources` must exist in top-level `content.sources:` or `content.raw.content_sources:`.

A plugin that references a non-existent resource fails the load with:

```
Plugin 'dev-tools': agents.include references unknown agent 'planner'
```

This means you cannot ship a plugin without the resources it binds. Conversely, you cannot "orphan" a resource behind a plugin — the top-level map owns the definition, the plugin just wires it up.

---

## Pure-Loader Contract

`ConfigLoader` (Phase 3b) is **pure**:

- `ConfigLoader::load()`, `load_from_path`, and `load_from_content` read files and produce a `ServicesConfig`. They **never write**.
- There is no `discover_and_load_agents` anymore. Agent discovery by directory scanning has been deleted. If you want an agent loaded, add an `include:` entry for its file.
- There is no `ConfigWriter::add_include`. Users edit `services.yaml` (or a generator tool does) to register new includes explicitly.
- `settings.apply_env_overrides()` is still called post-merge for runtime toggles, but the on-disk YAML is untouched.

The only consolidated loader is `ConfigLoader`. `EnhancedConfigLoader` has been deleted — its recursive-include capability is now the default behaviour of `ConfigLoader`.

---

## What Changed (0.1.x → Phase 1–3)

For maintainers upgrading existing profiles or tooling:

| Change | Before | After |
|---|---|---|
| Loader | `ConfigLoader` + `EnhancedConfigLoader` | Single `ConfigLoader` |
| Includes | Single-level, no cycle check | Recursive, cycle-detected, referrer-attributed errors |
| Unknown fields | Silently dropped | Hard error via `deny_unknown_fields` |
| `skills` / `content` | Side-loaded from separate discovery paths | First-class fields on `ServicesConfig` |
| `web` | Stub `WebConfig` / `FullWebConfig` / `WebBrandingConfig` | `systemprompt_provider_contracts::WebConfig` |
| Agent discovery | `discover_and_load_agents` auto-scan | Explicit `includes:` only |
| Config mutation | `ConfigWriter::add_include` on load | Loader never writes; explicit edits |
| Plugins | Free-form references, no validation | Binding descriptors; `validate()` enforces resolution |

### Migration checklist

1. Replace any `EnhancedConfigLoader::...` call sites with `ConfigLoader::...`.
2. Remove any call to `discover_and_load_agents` — add explicit `includes:` entries instead.
3. Remove any call to `ConfigWriter::add_include` — edit `services.yaml` directly.
4. Audit YAML files for unknown keys; `deny_unknown_fields` will surface typos on first load.
5. Ensure every plugin's `agents.include`, `skills.include`, `mcp_servers`, and `content_sources` entries correspond to real top-level resources.
6. Replace stub `WebConfig` / `FullWebConfig` / `WebBrandingConfig` references with `systemprompt_provider_contracts::WebConfig`.

---

## Key Source Files

| File | Purpose |
|---|---|
| `crates/infra/loader/src/config_loader.rs` | `ConfigLoader`, include recursion, cycle detection |
| `crates/shared/models/src/services/mod.rs` | `ServicesConfig`, `PartialServicesConfig`, `validate()` |
| `crates/shared/models/src/services/plugin.rs` | `PluginConfig`, `PluginComponentRef` |
| `crates/shared/models/src/services/skills.rs` | `SkillsConfig`, `SkillConfig` |
| `crates/shared/models/src/services/content.rs` | `ContentConfig` |
| `crates/shared/provider-contracts/src/web.rs` | Canonical `WebConfig` |
