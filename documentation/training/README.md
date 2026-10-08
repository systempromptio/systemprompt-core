# systemprompt Partner Certification Programme

Training and certification for partners and customers who sell, implement and operate the systemprompt platform inside a business.

The platform is governance infrastructure. It sits underneath the AI tools a company already uses (Claude Code, Claude Cowork, SDK applications, MCP hosts, Slack and Teams) and gives the organisation one place to decide who can use which models, skills, agents and MCP servers, and one record of what actually happened. That single layer shows up differently in every department. Sales sees governed skills grounded in CRM data. Engineering sees a metered, audited gateway in front of its coding agents. Security sees a default-deny authorisation hook and a trace for every action.

The programme is organised around those departmental outcomes. We call them pillars.

## The seven pillars

| # | Pillar | Business owner | What the business gets |
|---|--------|----------------|------------------------|
| 1 | [Revenue & Growth](pillars/01-revenue-growth.md) | CRO, CMO | Sales and marketing skills that run against live CRM and content data, consistently, for every rep |
| 2 | [People & Performance](pillars/02-people-performance.md) | CHRO, COO | Performance reviews, goals and productivity analytics built on evidence from the systems where work happens |
| 3 | [Engineering Productivity](pillars/03-engineering-productivity.md) | CTO, VP Engineering | Coding agents behind one governed gateway, with per-team usage, cost and tool audit |
| 4 | [Governance, Security & Compliance](pillars/04-governance-security.md) | CISO, DPO, General Counsel | Identity-bound AI access, default-deny policy, credential screening and an end-to-end audit trail on your own infrastructure |
| 5 | [Customer Operations](pillars/05-customer-operations.md) | VP Support, VP Customer Success | Agents in Slack and Teams that answer from governed knowledge and hand over cleanly to people |
| 6 | [AI FinOps & Platform Operations](pillars/06-ai-finops-operations.md) | CFO, Head of Platform | Spend by user, team, model and provider, quotas that enforce budgets, and provider choice without code changes |
| 7 | [Knowledge & Content](pillars/07-knowledge-content.md) | COO, Head of Enablement | Institutional know-how packaged as versioned skills and plugins, distributed by role |

A cross-pillar technical track, [Solution Architect](tracks/solution-architect.md), covers extensions, MCP servers, A2A agents, deployment and security design.

The [benefits matrix](benefits-matrix.md) puts every pillar's benefits, KPIs and enabling capabilities on one page. Use it in discovery workshops.

## Value framework

How each pillar is mapped from connected data to the bottom line:

- [Value model](value-model.md): **data source + implementation + control = expected benefit**, the five bottom-line levers (Revenue, Performance, Efficiency, Cost control, Risk), value formulas and every value chain.
- [Connectors catalogue](connectors.md): every data source to connect, its default access and control, and the pillars it feeds.
- [KPI catalogue](kpis.md): every leading and lagging KPI by lever, with its bottom-line translation.

## Certifications

The [certification schema](certification-schema.md) defines the credentials, exam blueprints, prerequisites, maintenance and partner tiers.

```
                        L5  Solution Architect
                                 ▲
                        L4  Consultant
                                 ▲
        L2  Administrator   L3  Pillar Specialist (one per pillar)
                    ▲            ▲
                    └── L1  Associate ──┘

        Sales Accreditation (parallel, non-technical)
```

Individuals earn credentials. Partners earn tiers and pillar specialisations from the credentials their people hold and the customer deployments they can reference.

## How to use this material

- **Partner sales teams**: read the benefits matrix and the "Business problems solved" and "Benefits delivered" sections of each pillar. That is the Sales Accreditation syllabus.
- **Functional consultants**: work through the pillar you deliver end to end, including the implementation pattern and the lab tasks in its certification objectives.
- **Platform administrators**: start with [getting-started.md](../getting-started.md) and [guides/configure.md](../guides/configure.md), then the Administrator blueprint in the schema.
- **Architects**: the Solution Architect track, then [concepts/](../concepts/) and [security/](../security/).

## A note on numbers

Nothing in this programme promises a percentage improvement. Every benefit is written as a mechanism (what changes), the capability that makes it possible, and a KPI the partner baselines before rollout and measures after. Customer results belong in the customer's own case study, measured on their data. Where a number is needed for a sales asset, use `[INSERT: measured result from customer deployment]` and fill it from a real engagement.
