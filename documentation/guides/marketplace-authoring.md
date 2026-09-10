# Authoring a marketplace

How to author a marketplace in Claude Code's `.claude-plugin` format and import it into a services tree, using strict sidecars for the settings Anthropic's format has no slot for.

A marketplace repository is authored the way Claude Code expects: a `marketplace.json`, one `plugin.json` per plugin, and `SKILL.md` files. It stays directly installable by Claude Code. The importer reads that tree and writes the services tree systemprompt loads, deriving everything Anthropic's format already states and taking the rest from two optional sidecar files.

The governing rule is that no fact has two authors. A field the importer can derive from the Anthropic manifests is **forbidden** in a sidecar and the import fails if it appears. Derived fields and sidecar fields are a disjoint union, never a merge.

## Prerequisites

- A marketplace repository in `.claude-plugin` format, or an existing one to adapt.
- A destination directory that does not exist or is empty. The importer composes a whole tree and cannot reason about what a partial earlier import left behind; re-importing means removing the destination first.
- The `systemprompt` CLI.

## 1. Lay out the repository

```
your-marketplace/
├── .claude-plugin/
│   ├── marketplace.json          # required
│   └── systemprompt.yaml         # optional marketplace sidecar
├── plugins/
│   └── alpha-tools/
│       ├── .claude-plugin/
│       │   ├── plugin.json       # required
│       │   └── systemprompt.yaml # optional plugin sidecar
│       ├── skills/
│       │   └── alpha_discovery/
│       │       ├── SKILL.md
│       │       └── checklist.md  # siblings are copied verbatim
│       ├── rules/
│       │   └── handover.md
│       ├── hooks/
│       │   └── hooks.json
│       └── scripts/
│           └── setup.sh
├── rules/                        # repository-level rules, see step 5
└── systemprompt/                 # the base tree, see step 6
```

The importer writes a mirror-image services tree:

| Source | Destination |
|--------|-------------|
| `.claude-plugin/marketplace.json` | `marketplaces/<id>/config.yaml` |
| `plugins/<id>/.claude-plugin/plugin.json` | `plugins/<id>/config.yaml` |
| `plugins/<id>/skills/<id>/` | `skills/<id>/` (config plus every file verbatim) |
| `rules/<name>.md` | `rules/<name>/{config.yaml,index.md}` |
| `plugins/<id>/hooks/hooks.json` | `hooks/<plugin>__<Event>__<n>/config.yaml` |
| `plugins/<id>/scripts/` | `plugins/<id>/scripts/` |
| `systemprompt/<dir>/` | `<dir>/` verbatim |

## 2. Write `marketplace.json`

Only `name` is required. It becomes the marketplace id, so it must be 3 to 50 characters of lowercase letters, digits and hyphens.

```json
{
  "name": "acme-field",
  "owner": { "name": "Acme Field Team", "email": "field@acme.example" },
  "metadata": {
    "description": "Field engineering tooling for the Acme delivery group.",
    "version": "2.1.0"
  },
  "plugins": [
    {
      "name": "alpha-tools",
      "source": "./plugins/alpha-tools",
      "description": "Discovery and reporting for field engagements.",
      "version": "1.4.0",
      "keywords": ["field", "discovery"]
    },
    {
      "name": "beta-reports",
      "source": "./plugins/beta-reports",
      "version": "0.9.0",
      "category": "reporting"
    }
  ]
}
```

A plugin entry may also carry `author`, `license`, `homepage`, `repository`, `tags` and `strict`. `source` must be a relative path inside the repository; a git or object source names something the importer cannot read and is refused. When `source` is absent the plugin is looked for under `metadata.pluginRoot`, defaulting to `./plugins/<name>`.

`license` defaults to `proprietary` when neither the plugin manifest nor the marketplace entry states one. `metadata.version` defaults to `0.1.0`.

## 3. Write the sidecars

Both sidecars are optional; absent means every default applies. Both reject unknown keys outright, and both require `schema: 1` as their first key. The schema number is the sidecar *format* version, not a content version, and an unrecognised value is refused rather than guessed at.

### Marketplace sidecar

`.claude-plugin/systemprompt.yaml`:

```yaml
schema: 1
marketplace:
  visibility: private           # public | private | org   (default: public)
  enabled: true                 # default: true
  access:
    default_included: false     # default: false
    roles: []
    rules:
      - rule_type: group        # any subject dimension except role and user
        values: [field]
        access: allow           # allow | deny  (default: allow)
        justification: Field delivery group tooling
    attributes: {}
  mcp_servers: { source: explicit, include: [knowledge-bank] }
  agents:      { source: explicit, include: [] }
  artifacts:   { source: explicit, include: [] }
```

`access` is the authz assignment block. `roles` is matched against the caller's roles; each entry in `rules` projects one further subject dimension. Roles have their own list, so `rule_type` may name neither `role` nor `user`. `attributes` is an opaque bag forwarded to extension authz hooks and never interpreted.

`mcp_servers`, `agents` and `artifacts` reference platform-defined entities **by id** from the base tree. They are not defined here.

### Plugin sidecar

`plugins/<id>/.claude-plugin/systemprompt.yaml`:

```yaml
schema: 1
plugin:
  category: business            # required unless the marketplace entry sets it
  enabled: true                 # default: true
  mcp_servers: { source: explicit, include: [knowledge-bank] }
  agents:      { source: explicit, include: [] }
  artifacts:   { source: explicit, include: [] }
  content_sources: {}
  rules: { source: explicit, include: [] }
  hooks: { governance: false, comms: false, include: [] }
  scripts:
    - name: setup
      source: scripts/setup.sh
```

Every script a sidecar declares must exist; a missing file fails the import rather than being skipped.

### The forbidden keys

These nine keys are rejected in either sidecar, by name, with an error saying where the value actually comes from:

| Key | Derived from |
|-----|--------------|
| `id` | the manifest `name` |
| `name`, `description`, `version` | the manifest fields of the same name |
| `author` | `owner` in `marketplace.json`, `author` in `plugin.json` |
| `keywords`, `license` | the manifest, falling back to the marketplace entry |
| `plugins` | the `plugins[]` array |
| `skills` | every `skills/*/SKILL.md` on disk |

This is why a sidecar carries no version of its own: the content version lives in the manifest, and the sidecar is versioned with its plugin by living in the same commit.

## 4. Skills

Each `skills/<id>/SKILL.md` becomes `skills/<id>/config.yaml`, and every file and subdirectory beside it is copied unchanged.

The **directory name is the id**, never the frontmatter `name`. The loader keys skills by directory and refuses a descriptor that disagrees with it, while Anthropic's `name` is a display string that may contain anything.

Skill ids are canonically `snake_case`, but a generated Claude Code bundle names the directory in `kebab-case`. The importer maps `-` to `_` on the directory name. A snake id contains no hyphens, so that inverts the projection exactly and a bundle generated from a services tree imports back to the ids it started from.

Frontmatter `description` must be present and non-empty. `name`, `tags`, `category` and `hosts` are optional; `tags` accepts a list or a comma-separated string. A skill with no `category` inherits its plugin's.

A skill id claimed by two plugins is an error. Skill ids are unique across the whole tree, not per plugin.

## 5. Rules and hooks

A rule is a markdown file a plugin ships. `plugins/<id>/rules/*.md` become `rules/<name>/config.yaml` plus the markdown as `index.md`, and each rule id is added to that plugin's `rules.include`. Rule text is trimmed before it is hashed, so a file differing only by a trailing newline does not change the plugin's content version.

Rules at the **repository root** attach to no plugin. A rule only reaches a host through a plugin that lists it, so a root rule is imported but inert: a warning normally, an error under `--strict`. Put rules under the plugin that should carry them.

Hooks come from `plugins/<id>/hooks/hooks.json` in Claude Code's shape. Claude Code groups hooks by event and matcher with several actions under each; the systemprompt catalogue is flat, one directory per command. The importer expands the cross product into `hooks/<plugin>__<Event>__<n>/`, naming each after the plugin so two plugins binding the same event cannot collide, and adds the ids to that plugin's `hooks.include`. Only `command` actions are importable; a prompt- or agent-typed action has no command to run and is reported.

## 6. The base tree

`systemprompt/` is copied verbatim into the destination. It carries the platform-defined halves an Anthropic repository cannot express: MCP server definitions, agents, gateway and governance configuration.

| Base-only, allowed under `systemprompt/` | Authored in Anthropic form, refused under `systemprompt/` |
|---|---|
| `access-control`, `agents`, `ai`, `config`, `content`, `external_agents`, `gateway`, `governance`, `mcp`, `scheduler`, `slack`, `web` | `marketplaces`, `plugins`, `skills`, `rules`, `hooks`, `artifacts` |

A directory in neither list is an error naming it. A repository may consist of nothing but a base tree and no Anthropic content at all.

If the base tree supplies its own `config/config.yaml`, that file owns the `includes:` list. If it does not, the importer writes a minimal aggregator naming every top-level YAML file in the directories it copied, so the destination loads without hand-editing.

## 7. Import

```bash
systemprompt core marketplace import --from ./your-marketplace --into ./services
```

Add `--dry-run` to compute the full report and write nothing. Dry run also lifts the empty-destination requirement, so it is safe to run against a directory you intend to keep.

Add `--strict` to refuse a tree that only half-translates:

| Condition | Default | `--strict` |
|---|---|---|
| No `marketplace.json` | warning | error |
| Inline `mcpServers` in `plugin.json`, or a `.mcp.json` | warning | error |
| A `commands/` directory | warning | error |
| No category on the sidecar or the marketplace entry | warning, `general` applied | error |
| A plugin `source` that is not a local path | warning, plugin skipped | error |
| Root-level rules belonging to no plugin | warning | error |
| An `agents/` directory | warning | warning |
| A non-command hook action | warning | warning |
| A plugin shipping no skills | warning | warning |

A forbidden sidecar key, a skill id claimed twice, a rule name claimed twice, a missing script, a malformed manifest, and an id that fails validation are always errors, in both modes.

## 8. Versioning

Three things carry a version and each has exactly one owner.

- **Content** — `plugin.json` `version` and `marketplace.json` `metadata.version`. Authored by you, derived by the importer, forbidden in sidecars.
- **Sidecar format** — `schema: 1`. Required, and the only version a sidecar carries.
- **Bundle content** — computed, not authored. A generated plugin bundle appends a content hash to the plugin version as semver build metadata, so `1.4.0` becomes `1.4.0+<hash>`. Changing any file a plugin ships, including a rule, changes it.

Two facts are not representable in Anthropic's format and are lost on a round trip: a plugin's display name, which becomes its id, and YAML comments, which is why the sidecars are copied through verbatim rather than regenerated.

## Worked example

The repository sketched in step 1, with the `marketplace.json` of step 2 and the sidecars of step 3, imports to this tree:

```
services/
├── marketplaces/acme-field/config.yaml
├── plugins/
│   ├── alpha-tools/config.yaml
│   ├── alpha-tools/scripts/setup.sh
│   └── beta-reports/config.yaml
├── skills/
│   ├── alpha_discovery/{config.yaml,SKILL.md,checklist.md}
│   ├── alpha_report/{config.yaml,SKILL.md}
│   └── beta_summary/{config.yaml,SKILL.md}
├── rules/
│   ├── handover/{config.yaml,index.md}
│   └── security/{config.yaml,index.md}
├── hooks/
│   ├── alpha-tools__PreToolUse__0/config.yaml
│   └── alpha-tools__SessionStart__0/config.yaml
├── mcp/knowledge-bank.yaml
└── config/config.yaml
```

`marketplaces/acme-field/config.yaml` shows the join clearly. Everything above `visibility` is derived from `marketplace.json`; everything from `visibility` down comes from the sidecar:

```yaml
marketplace:
  id: acme-field
  name: acme-field
  description: Field engineering tooling for the Acme delivery group.
  version: 2.1.0
  author:
    name: Acme Field Team
    email: field@acme.example
  license: proprietary
  plugins:
    source: explicit
    include: [alpha-tools, beta-reports]
  enabled: true
  visibility: private
  mcp_servers:
    source: explicit
    include: [knowledge-bank]
  access:
    default_included: false
    rules:
      - rule_type: group
        values: [field]
        access: allow
        justification: Field delivery group tooling
```

Three things in that example are worth tracing:

- `alpha-tools` has `category: business` from its sidecar; `beta-reports` has no sidecar and takes `reporting` from its marketplace entry.
- `handover` is a rule under `alpha-tools`, so it lands in that plugin's `rules.include`. `security` sits at the repository root, so it is imported but belongs to no plugin and is reported.
- `mcp/knowledge-bank.yaml` came from `systemprompt/mcp/`, which is why the sidecars can reference `knowledge-bank` by id.

## Verify

```bash
systemprompt core marketplace import --from ./your-marketplace --into ./services --dry-run --strict
```

A clean strict dry run means the tree translates completely. The report lists every marketplace, plugin, skill, rule and hook it would write, the base directories it would copy, and any warnings.

## Related pages

- [configure.md](configure.md) — the profile that points at a services tree.
- [authoring-extensions.md](authoring-extensions.md) — adding platform capability in Rust.
- [../concepts/mcp.md](../concepts/mcp.md) — the MCP servers a marketplace references by id.
