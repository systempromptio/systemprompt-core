# Certification schema

How individuals are certified on the systemprompt platform, and how partner organisations earn tiers and pillar specialisations from those certifications.

The structure borrows from two programmes partners already know. From Salesforce: role-based credentials that stack (Associate, Administrator, Specialist, Consultant, Architect) and a mandatory maintenance cycle. From Odoo: certification tied to a platform version, and partner tiers gated on certified headcount plus live, referenceable customers rather than on spend alone.

Commercial terms (fees, discount bands, lead allocation) are marked `[COMMERCIAL: ...]` and are confirmed separately from this document.

## 1. Individual credentials

| Level | Credential | Code | Audience | Prerequisites | Assessment |
|-------|-----------|------|----------|---------------|------------|
| L1 | Certified Associate | `SP-ASC` | Anyone selling, delivering or using the platform | None | 60 multiple-choice questions, 90 minutes, 70% to pass |
| L2 | Certified Administrator | `SP-ADM` | Platform owners and operators | `SP-ASC` | 60 questions (70% to pass) plus a 2-hour hands-on lab |
| L3 | Pillar Specialist | `SP-SPx-<pillar>` | Functional consultants | `SP-ASC` | 45 questions (70% to pass) plus a pillar lab |
| L4 | Certified Consultant | `SP-CON` | Delivery leads, engagement managers | `SP-ADM` and two `SP-SPx` | Scenario exam plus a capstone implementation review |
| L5 | Certified Solution Architect | `SP-ARC` | Technical and security architects | `SP-ADM`, `SP-CON` | Written design submission plus a 60-minute board review |
| n/a | Sales Accreditation | `SP-SAL` | Partner account executives and pre-sales | None | 30 questions plus a recorded discovery role-play |

Pillar codes: `REV` Revenue & Growth, `PPL` People & Performance, `ENG` Engineering Productivity, `GOV` Governance, Security & Compliance, `CXO` Customer Operations, `FIN` AI FinOps & Platform Operations, `KNW` Knowledge & Content. Example: `SP-SPx-GOV`.

### L1 Certified Associate

Proves a candidate can explain what the platform is, what it is not, and where each pillar fits in a business.

| Domain | Weight |
|--------|--------|
| Platform fundamentals: self-hosted control plane, profiles, PostgreSQL as the only durable state, the five capabilities (gateway, MCP, A2A, OAuth2/OIDC, extensions) | 25% |
| Governance model: identity-bound requests, default-deny authorisation hook, rate limits, audit and `trace_id` | 20% |
| Building blocks a business uses: skills, plugins, agents, MCP servers, connectors, marketplaces | 20% |
| The seven pillars: problems, benefits, KPIs | 25% |
| Client surfaces: Claude Code, Claude Cowork via the bridge, SDK applications, Slack and Teams | 10% |

Sources: [overview.md](../overview.md), [concepts/](../concepts/), the pillar documents in this folder.

### L2 Certified Administrator

Proves a candidate can stand up, configure and run a governed tenant.

| Domain | Weight |
|--------|--------|
| Profiles and secrets: `profile.yaml`, secrets sources, Vault/OpenBao, bootstrap order | 15% |
| Identity: OIDC SSO to the customer IdP, WebAuthn, roles, users and sessions (`admin users`) | 15% |
| Gateway: providers, routes, model catalogue, quotas and policy | 20% |
| Marketplaces: authoring, importing and scoping plugins, skills, agents and MCP servers to people and teams | 20% |
| Bridge enrolment and desktop clients | 10% |
| Operations: health probes, Prometheus metrics, structured logs, OTLP export, upgrade and rollback | 10% |
| Analytics: `analytics overview`, `costs`, `requests`, `tools`, `agents`, `sessions` | 10% |

Lab (2 hours, scored on the resulting tenant):
1. Configure a profile with OIDC SSO and two roles.
2. Add two upstream providers and route model patterns to each.
3. Import a marketplace and scope one plugin to a single team.
4. Set a quota for one team and demonstrate a rejected request once it is exhausted.
5. Produce a cost and tool-usage readout for the lab period from the CLI.

Sources: [guides/configure.md](../guides/configure.md), [guides/configure-providers.md](../guides/configure-providers.md), [guides/marketplace-authoring.md](../guides/marketplace-authoring.md), [guides/operate.md](../guides/operate.md), [reference/cli.md](../reference/cli.md).

### L3 Pillar Specialist

One credential per pillar. Each pillar document ends with a "Certification objectives" section that is the exam blueprint and lab for that specialism. Every Specialist exam shares the same frame:

| Domain | Weight |
|--------|--------|
| Business discovery: the pillar's buyer, problems and baseline KPIs | 20% |
| Solution design: which skills, agents, MCP servers and policies solve which problem | 30% |
| Configuration and rollout on the platform | 25% |
| Governance guardrails specific to the pillar | 15% |
| Measuring and evidencing value: the pillar value map, levers and value formulas | 10% |

The pillar document refines the content of each domain.

### L4 Certified Consultant

Proves a candidate can lead a multi-pillar engagement from discovery to measured value.

- **Scenario exam** (3 hours): a written case. A fictional company with a stated org chart, toolset and compliance regime. The candidate produces a discovery readout, a pillar prioritisation, a rollout sequence and a measurement framework.
- **Capstone review**: the candidate presents one real engagement they led (customer details may be anonymised) to two certified Architects. Pass criteria: a documented baseline, a configuration that matches the design, governance guardrails in place, and measured KPI movement against baseline.

### L5 Certified Solution Architect

Proves a candidate can design the platform for a complex organisation and defend the design to a CISO. Syllabus in [tracks/solution-architect.md](tracks/solution-architect.md).

- **Design submission**: architecture for a supplied brief covering deployment topology, identity, provider routing, extension and MCP design, data residency and the audit story.
- **Board review**: 60 minutes with two Architects and one security reviewer. Candidates are expected to say what the platform does not govern (for example, client-side tool execution that is not routed through the gateway or a governed MCP server) as clearly as what it does.

### Sales Accreditation

For people who sell, not configure.

| Domain | Weight |
|--------|--------|
| Positioning: governance infrastructure, build vs buy, ownership | 25% |
| Pillar value stories and their KPIs | 40% |
| Discovery: finding the pillar with the clearest baseline | 20% |
| Boundaries: what the platform is not (not a hosted chatbot, not a sidecar, governance applies to routed traffic) | 15% |

Role-play: a recorded 15-minute discovery call with a mock buyer for one pillar, scored on questions asked and baseline KPIs identified.

## 2. Versioning and maintenance

- **Version-bound.** Every credential records the platform minor version it was examined on (for example `0.62`). Exams are refreshed on each minor release that changes an examined surface.
- **Annual maintenance.** Holders complete a free online maintenance module each year covering changes since their exam version. Missing it moves the credential to `lapsed`. A lapsed credential stops counting toward partner tiers and is restored by completing the outstanding module within 12 months; after that the exam is retaken.
- **Retakes.** A failed exam may be retaken after 14 days, then after 30 days. `[COMMERCIAL: retake fee]`
- **Verification.** Each credential gets a unique ID (`SP-ADM-2026-000123`) that a customer can verify on a public lookup page. `[COMMERCIAL: verification portal]`

## 3. Partner tiers

Tiers are earned on evidence, re-evaluated every 12 months. Only active (not lapsed) credentials count.

| Requirement | Registered | Silver | Gold | Platinum |
|-------------|-----------|--------|------|----------|
| Signed partner agreement | Yes | Yes | Yes | Yes |
| Certified Associates | 2 | 4 | 8 | 15 |
| Certified Administrators | 0 | 1 | 3 | 6 |
| Pillar Specialists (distinct pillars covered) | 0 | 1 | 3 | 5 |
| Certified Consultants | 0 | 0 | 1 | 3 |
| Certified Solution Architects | 0 | 0 | 1 | 2 |
| Live customer deployments (production, 90 days+) | 0 | 1 | 4 | 10 |
| Referenceable customers | 0 | 1 | 2 | 5 |
| Customer satisfaction (post-project survey, average) | n/a | 4.0 / 5 | 4.2 / 5 | 4.5 / 5 |

Tier benefits (to be confirmed commercially):

| Benefit | Registered | Silver | Gold | Platinum |
|---------|-----------|--------|------|----------|
| Partner portal, training access, demo tenant | Yes | Yes | Yes | Yes |
| Margin / discount band | `[COMMERCIAL]` | `[COMMERCIAL]` | `[COMMERCIAL]` | `[COMMERCIAL]` |
| Listed in partner directory | n/a | Yes | Featured | Featured |
| Inbound lead sharing | n/a | n/a | Yes | Priority |
| Roadmap briefings | n/a | n/a | Quarterly | Monthly, with design input |
| Co-marketing funds | n/a | n/a | `[COMMERCIAL]` | `[COMMERCIAL]` |
| Exam vouchers per year | 2 | 5 | 15 | 30 |

## 4. Pillar specialisations

A specialisation is a badge a partner displays next to its tier. It tells a buyer "this partner has done this, for this department, more than once".

To earn a pillar specialisation a partner needs:
- 2 active Pillar Specialists for that pillar (3 for `GOV`);
- 2 production deployments where that pillar is in scope, each with a documented baseline and a post-rollout measurement of at least one KPI from the pillar document;
- 1 of those customers willing to act as a reference.

`GOV` carries the stricter requirement because a governance deployment is usually what a customer's security team will scrutinise in procurement.

A partner may hold any number of specialisations at any tier. Platinum requires at least three.

## 5. Learning paths

| Role | Path |
|------|------|
| Account executive | `SP-SAL`, then `SP-ASC` |
| Functional consultant | `SP-ASC` → one or two `SP-SPx` → `SP-CON` |
| Platform administrator | `SP-ASC` → `SP-ADM` → `SP-SPx-FIN` |
| Security consultant | `SP-ASC` → `SP-ADM` → `SP-SPx-GOV` → `SP-ARC` |
| Developer / integrator | `SP-ASC` → `SP-ADM` → `SP-SPx-ENG` → `SP-ARC` |
