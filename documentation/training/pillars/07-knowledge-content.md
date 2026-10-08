# Pillar 7: Knowledge & Content

The organisation's methods, playbooks and expertise captured as skills, plugins and agents that are versioned, owned, distributed to the right roles and measured. Knowledge that used to live in a few experts' heads, or in a wiki nobody reads, becomes something people actually run.

## 1. Who it's for

- **Buyer**: COO, Chief of Staff, Head of Enablement, Head of Knowledge Management, CMO (for content operations).
- **Users**: subject-matter experts who author skills, enablement and L&D teams, content and web teams, every employee as a consumer.
- **Partner role delivering it**: consultant holding `SP-SPx-KNW`, often alongside the Revenue or Customer Operations specialist.

## 2. Business problems solved

1. Expertise does not scale. The best way to do a task is known by a few people and written down nowhere usable.
2. Prompts are copied between chats and documents; nobody knows which version is current or correct.
3. When a process changes, there is no way to update how everyone's AI does it.
4. Every department buys or builds its own AI content, duplicating effort.
5. Published content (web, documentation, campaigns) is slow to produce and inconsistent.
6. When an expert leaves, their know-how leaves with them.

## 3. Benefits delivered

| Benefit | Mechanism | Enabling capability | KPI | How to baseline |
|---------|-----------|---------------------|-----|-----------------|
| Expertise that scales | Experts author skills once; the marketplace distributes them to every role that needs them | Marketplace authoring and import, plugins, skills | Skills in production; users per skill | Inventory of existing prompts and playbooks |
| One current version | Plugins are versioned; desktops sync from a signed manifest; old versions are retired centrally | Marketplace versioning, bridge manifest sync | Users on the current version; time for a change to reach all users | Time to roll out the last process change |
| Right knowledge to the right role | Plugins, agents and MCP servers are included per marketplace and admitted per role | Marketplace membership and scoping, authorisation cascade | Irrelevant-plugin complaints; per-role catalogue size | Current catalogue |
| Less duplicated work | A shared catalogue shows what exists before someone builds it again | Marketplace catalogue, `core skills list`, `core plugins` | Duplicate skills retired; build requests deduplicated | Count of overlapping prompts across teams |
| Faster content production | Content, templates and web content types are managed on the platform with drafting and review skills | Content domain, `web content-types`, `web templates`, `analytics content` | Time from brief to publish; review rounds | Last quarter's content cycle time |
| Retained know-how | Skills and agents belong to the organisation, not to an individual's chat history | Organisation-owned marketplaces in the customer's repository | Critical workflows with a documented, owned skill | Workflow inventory |
| Evidence of what is used | Usage per skill and tool shows what to invest in and what to retire | `analytics tools`, `analytics conversations` | Skill usage trend; retired skills | Zero (no visibility today) |

## 4. Value map

How this pillar turns connected data into movement on the bottom line. Read every row of the value chain as **data source + implementation + control = expected benefit**, then follow the benefit to its lever and KPI. Levers and value formulas are defined in the [value model](../value-model.md); every connector is described in the [connectors catalogue](../connectors.md) and every KPI in the [KPI catalogue](../kpis.md).

### 4.1 Data sources to connect

| Data source | Example systems | Data used | Access | Connected via | Data owner |
|-------------|-----------------|-----------|--------|---------------|------------|
| Existing playbooks and SOPs | Wikis, PDFs, process docs, slide decks | Methods to capture as skills | Read (authoring input) | Skill authoring; MCP to the wiki | Process owners |
| Marketplace repositories | Git repositories holding plugins, skills, agents | Versioned skills, rules, hooks | Read and write (authors) | Marketplace import | Knowledge owners |
| Knowledge stores | Confluence, SharePoint, Notion, Google Drive | Reference content for agents | Read | MCP server | Knowledge owners |
| Web and content | CMS, the platform's content and web management | Pages, templates, content types | Read and write (drafts) | Content domain, `web` commands | Content team |
| Identity provider | Okta, Entra ID | Roles and teams for distribution | Read (OIDC) | OIDC SSO | IT |
| Platform usage | systemprompt analytics | Usage per skill, tool and content item | Read | Built in | Platform owner |

### 4.2 Value chain

| # | Data source (input) | + Implementation | + Control | = Expected benefit (output) | Lever |
|---|---------------------|------------------|-----------|-----------------------------|-------|
| 1 | Playbooks and SOPs | Experts and consultants convert top workflows into tested skills | Each skill has an owner, a review date and test inputs | Expertise used by everyone, not only the expert | Performance, Efficiency |
| 2 | Marketplace repositories + IdP roles | Plugins scoped to roles and synced to desktops by the bridge | Signed manifest; authorisation decides who sees what | Right method in front of the right role | Efficiency |
| 3 | Marketplace versioning | A process change is a new plugin version | Retire old versions centrally | Process changes reach everyone in days, not months | Performance, Risk |
| 4 | Knowledge stores | Agents answer "how do we do X here" from approved sources | Read-only; sources owned and dated | Fewer interruptions of experts; faster onboarding | Efficiency |
| 5 | Web and content | Drafting and review skills on managed content types and templates | Brand and legal review built into the skill | Faster content cycle; fewer review rounds | Efficiency, Cost control |
| 6 | Usage analytics | Quarterly catalogue review; retire unused and duplicate skills | Owner sign-off | Lean catalogue; investment goes to what is used | Cost control |

### 4.3 KPI map

| Lever | Leading KPI (input, moves first) | Lagging KPI (output, moves the P&L) | Bottom-line translation | Measured from |
|-------|----------------------------------|-------------------------------------|-------------------------|---------------|
| Efficiency | Skills in production; users per skill | Time per task for captured workflows | Δ time per task × task volume × loaded hourly cost | Time study, `analytics tools` |
| Performance | Users on current skill version | Error and rework rate on captured workflows | Δ rework hours × loaded hourly cost | Process quality data |
| Efficiency | Time for a change to reach all users | Time to adopt a new process or policy | Days saved × affected staff × daily cost of the old process | Marketplace versions, analytics |
| Cost control | Duplicate skills retired; content review rounds | Content and enablement spend | Δ agency and enablement spend | Finance |
| Risk | Critical workflows with an owned skill | Knowledge loss when people leave | Avoided ramp-up time for replacements × loaded cost | HRIS, skill inventory |

### 4.4 Expected benefit

**Net annual value = sum of the lever values in 4.3 − (platform cost + AI usage cost + delivery cost).** Every input comes from the customer's own baseline, taken in discovery (section 7), and is re-measured at 30, 60 and 90 days. Quote results only from measured data: `[INSERT: measured result from customer deployment]`.

## 5. Platform capabilities used

- **Marketplace authoring**: repository layout, `marketplace.json`, sidecars, skills, rules, hooks, import and versioning. See [guides/marketplace-authoring.md](../../guides/marketplace-authoring.md).
- **Services bundles** to package, sign and publish services trees. See [guides/services-bundles.md](../../guides/services-bundles.md).
- **Bridge** signed manifest sync to desktops.
- **Content and web** management (`core content`, `web content-types`, `web templates`) and content analytics. See [reference/cli.md](../../reference/cli.md).
- **A2A agents** that answer from curated knowledge. See [concepts/a2a-protocol.md](../../concepts/a2a-protocol.md).

## 6. Reference use cases

**Playbook to skill.** Before: the pricing-approval playbook is a 30-page PDF. After: an approval skill walks the user through it, checks inputs and produces the request in the required format; the pricing team owns and versions it.

**Process change rollout.** Before: a policy change is announced by email and adopted unevenly. After: the owning team updates the skill, bumps the plugin version and every user gets it on next sync.

**Expert departure.** Before: a senior analyst leaves and their method leaves with them. After: the method is a skill in the team marketplace with a named successor as owner.

**Content operations.** Before: web and documentation updates queue behind a small team. After: drafting and review skills encode structure and voice; content is managed and measured on the platform.

## 7. Implementation pattern

1. **Discover**: inventory existing prompts, playbooks and expert workflows; pick ten with the highest frequency or risk.
2. **Design the catalogue**: marketplaces by audience; plugin boundaries; owners and review dates; naming and versioning rules.
3. **Author**: pair each expert with a consultant to turn a workflow into a tested skill; package into plugins.
4. **Distribute**: import, scope to roles, sync to desktops via the bridge.
5. **Govern**: quarterly catalogue review using usage analytics; retire unused and duplicate skills.

Typical deliverables: knowledge inventory, catalogue design, authoring standard, first plugin set, ownership register, catalogue health dashboard.

## 8. Risks and governance guardrails

- **Ownership**: every skill has an owner and a review date, or it is not published.
- **Quality**: skills are tested against example inputs before release; treat them like code (review, version, changelog).
- **Sensitive knowledge**: scope confidential playbooks to the roles that may see them; the marketplace include list is not an access decision on its own, the authorisation cascade is.
- **Catalogue bloat**: more skills is not better. Measure use and retire aggressively.

## 9. Certification objectives (`SP-SPx-KNW`)

Candidates can:
- Run a knowledge inventory and prioritise workflows to capture.
- Author a skill with correct frontmatter and package it in a plugin with sidecars.
- Design a multi-marketplace catalogue and explain how membership and authorisation interact.
- Version and roll out a change, and confirm users received it.
- Produce a catalogue health readout from usage analytics.
- Build the value map for a customer: data sources to connect, implementation, controls, levers, and the value formula filled with the customer's own baseline.

Lab: convert a supplied written playbook into a skill, package it in a plugin, import it into a demo marketplace, scope it to one role, release a second version, and show usage for both versions.
