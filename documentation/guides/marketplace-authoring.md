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
├── .systempromptignore           # optional, dev-only paths, see step 4
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
│       │       └── checklist.md  # siblings are copied, dev-only files aside
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
| `plugins/<id>/skills/<id>/` | `skills/<id>/` (config plus every file verbatim, dev-only files excluded) |
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

A plugin entry may also carry `author`, `license`, `homepage`, `repository`, `tags`, `skills` and `strict`. When `source` is absent the plugin is looked for under `metadata.pluginRoot`, defaulting to `./plugins/<name>`; a string `source` is a relative path inside the repository.

### Re-listing an upstream plugin

`source` may also be one of Claude Code's git forms, to re-list a plugin published in another repository:

```json
{
  "name": "b2c",
  "source": {
    "source": "git-subdir",
    "url": "SalesforceCommerceCloud/b2c-developer-tooling",
    "path": "skills/b2c",
    "ref": "b2c-agent-plugins@1.10.0",
    "sha": "efc7d4633dfb8fd05baeb8d96fa17f4bb26de498"
  },
  "strict": false,
  "category": "development"
}
```

`github` (`repo`), `url` (`url`) and `git-subdir` (`url` + `path`) are accepted; `url` is `owner/repository` on GitHub or a public `https` URL. The importer fetches the commit named by `sha` at import time and imports that subtree exactly like a local plugin, so the services tree and every bundle packed from it carry the upstream files: boot never fetches a plugin, and the instance serves upstream skills under the same access rules, hooks and analytics as its own. The import report's `upstream` row names each plugin with the commit it was taken from.

- Pin with `sha`. An entry with only a `ref` is imported from whatever that ref points at today and raises a warning, which `--strict` refuses; a publishing pipeline should always pin.
- The fetch is https-only, runs git with hooks disabled, refuses submodules, and is bounded at 256 files and 8 MiB per plugin.
- `strict: false` makes the marketplace entry the plugin's whole manifest, which is what an upstream folder without `.claude-plugin/plugin.json` needs.
- `skills` names where the skills live when not under `skills/`: a path or a list of paths inside the plugin, each either a skill folder or a folder of them (`["./"]` for skill folders at the plugin root).
- `npm` and `pip` sources cannot be vendored; the plugin is skipped with a warning.

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

### Plugin dependencies and Node packages

A plugin may depend on other plugins, in Claude Code's own `plugin.json` vocabulary. The importer keeps the list as `plugin.dependencies` in the plugin's `config.yaml`, and the generated bundle writes it back out unchanged:

```json
{
  "name": "storefront-migration",
  "dependencies": [
    "audit-logger",
    { "name": "b2c-cli", "marketplace": "salesforce", "version": "^2.0" }
  ]
}
```

A bare name resolves inside the same marketplace, so that plugin must be one the marketplace carries. A dependency that names another marketplace is accepted only when the marketplace allowlists the target in `allowCrossMarketplaceDependenciesOn` (in `marketplace.json`, exactly as Claude Code reads it) **and** the target is either another marketplace in the same services tree or declared in the marketplace sidecar:

```yaml
schema: 1
marketplace:
  external_marketplaces:
    - name: salesforce
      source: { source: github, repo: SalesforceCommerceCloud/claude-plugins }
    - name: partner-tools
      source: { source: git, url: https://git.example.com/partner/claude-plugins.git }
```

`version`, when present, must be a semver range. A dependency on an undeclared or unallowlisted marketplace fails validation: the gateway never emits a manifest that Claude Code would refuse to install on every user's machine.

The gateway never fetches an external marketplace. The bridge registers it with Claude Code (`extraKnownMarketplaces` in the user's `settings.json`), enables each foreign dependency at user scope, and writes the allowlist into the mirrored `marketplace.json`; Claude Code then clones and installs the dependency plugins itself at its next session start. Claude Desktop has no dependency model, so a plugin that declares dependencies raises a host warning there rather than installing them silently.

A plugin whose skills ship Node scripts keeps `package.json` and its lockfile at the plugin root. The importer copies both, the bundle ships them, and the bridge runs the same frozen, script-less install Claude Code would (`npm ci --ignore-scripts`, or `bun install --frozen-lockfile --ignore-scripts` for `bun.lock`) into the synced plugin, bounded to 60 seconds. The lockfile must be one Claude Code accepts — `bun.lock`, `bun.lockb`, `npm-shrinkwrap.json` or `package-lock.json`, in that priority order; `yarn.lock` and `pnpm-lock.yaml` are not used because their installers cannot skip lifecycle scripts, and a `package.json` with no accepted lockfile is reported at import and shipped without one. A failed or impossible install is a sync warning, never a failed sync, and an unchanged plugin is not reinstalled on the next sync.

## 4. Skills

Each `skills/<id>/SKILL.md` becomes `skills/<id>/config.yaml`, and every file and subdirectory beside it is copied unchanged, except dev-only files.

### Dev-only files

What a skill's author needs and its user does not never reaches a client. These are excluded by default, matched inside each skill folder:

- `README.md` at the skill root, in any case (a `README.md` in a subfolder such as `references/` ships);
- any `tests/`, `test/`, `fixtures/` or `__tests__/` directory, at any depth;
- any `*.test.*` or `*.spec.*` file, at any depth.

A kit adds its own paths in `.systempromptignore` at the repository root, in gitignore syntax: `#` comments, `*`, `?` and `**`, a trailing `/` for a directory, and a pattern containing `/` anchored to the repository root. The ignore file is read before the defaults, one directory level at a time, so `!fixtures/` ships a skill's fixtures; a negation cannot re-include a file under an excluded directory. A malformed pattern fails the import.

```
# .systempromptignore
*.snap
coverage/
/plugins/alpha-tools/skills/alpha_discovery/notes/
!fixtures/
```

The same rules apply where skill files are captured into a managed revision (the authoring tree's own `.systempromptignore` at its root) and the defaults apply again when a revision is laid out for a client, so a dev file left in an older revision is not installed either.

The **directory name is the id**, never the frontmatter `name`. The loader keys skills by directory and refuses a descriptor that disagrees with it, while Anthropic's `name` is a display string that may contain anything.

Skill ids are canonically `snake_case`, but a generated Claude Code bundle names the directory in `kebab-case`. The importer maps `-` to `_` on the directory name. A snake id contains no hyphens, so that inverts the projection exactly and a bundle generated from a services tree imports back to the ids it started from.

Frontmatter `description` must be present and non-empty. `name`, `tags`, `category` and `hosts` are optional; `tags` accepts a list or a comma-separated string. A skill with no `category` inherits its plugin's.

A skill id claimed by two plugins is an error. Skill ids are unique across the whole tree, not per plugin.

### Skill frontmatter

The platform owns seven frontmatter keys and passes every other key through as you wrote it:

| Key | What the platform does with it |
|-----|--------------------------------|
| `name` | Replaced in the client `SKILL.md` by the kebab-case skill id. |
| `description` | Re-emitted from the skill's `config.yaml`. |
| `title` | The display name; stripped from the client file. |
| `tags`, `category`, `display_category` | Catalogue metadata; stripped. |
| `hosts` | Which clients receive the skill; stripped. |

Any other key (`allowed-tools`, `disable-model-invocation`, `user-invocable`, `argument-hint`, `when_to_use`, `model`, `hooks`, `metadata`, a key Claude Code adds in a later release) is kept in `config.yaml` under `frontmatter`, in the order you wrote it, and written to the `SKILL.md` that Claude Code, Claude Desktop and Cowork receive, after `name` and `description`. Nested values such as `hooks` and `metadata` keep their structure. The value is re-serialised, not copied byte for byte, so comments and quoting style in the frontmatter are not preserved.

```yaml
---
name: alpha-discovery
title: Alpha Discovery          # platform-owned, stripped
description: Walk a new field engagement.
allowed-tools: [Read, Grep]     # passed through
disable-model-invocation: true  # passed through
---
```

The frontmatter travels in the signed manifest as JSON, so a mapping key must be a string, a value must not carry a YAML tag (`!tag`) and a number must be finite; anything else fails the import. A skill's `sha256` covers the passed-through keys as well as its instructions, so editing an authored key re-stamps the skill. Codex, OpenCode and Hermes skill files keep `name` and `description` alone.

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
| A plugin `source` the importer cannot vendor (`npm`, `pip`, unrecognised) | warning, plugin skipped | error |
| A git plugin `source` without a `sha` | warning, imported from its ref | error |
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
