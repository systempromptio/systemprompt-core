# Benefits matrix

One page for discovery workshops: what each pillar changes in the business, the platform capability behind it, and the KPI to baseline before rollout. Detail and implementation patterns are in each pillar document.

Every pillar rests on the same foundation: identity-bound requests, a default-deny authorisation hook, one gateway, governed MCP servers and a correlated audit trail. That shared layer is why a customer who starts with one pillar can add the next without a new platform.

| Pillar | Headline benefit | Supporting benefits | Key capabilities | Primary KPIs |
|--------|------------------|---------------------|------------------|--------------|
| [1. Revenue & Growth](pillars/01-revenue-growth.md) | Every rep and marketer runs the organisation's best methods against live data | More selling time; earlier deal-risk detection; better CRM hygiene; on-brand content at volume | Skills and plugins by role, CRM/email/calendar MCP servers, gateway, usage analytics | Selling time per rep, forecast accuracy, opportunity field completeness, content throughput |
| [2. People & Performance](pillars/02-people-performance.md) | Performance and productivity managed on evidence, not memory | Evidence-based reviews; continuous goal tracking; early overload warning; grounded capacity planning; governed people data | MCP servers to HRIS and work systems, per-user authorisation, review and goal skills, scheduled jobs, audit, OTLP export | Manager hours per review cycle, goal freshness, team throughput and cycle time, regretted attrition |
| [3. Engineering Productivity](pillars/03-engineering-productivity.md) | All coding tools behind one governed, metered endpoint | Shared standards; safe agent tool access; credentials kept out of prompts; provider choice | Gateway `/v1`, bridge, MCP servers, plugins with rules and hooks, quotas, OTLP | Traffic through gateway, AI cost per engineer, DORA metrics, PR cycle time |
| [4. Governance, Security & Compliance](pillars/04-governance-security.md) | Identity, enforcement and audit on every governed AI action, on customer infrastructure | Shadow-AI reduction; reconstructable incidents; faster questionnaires; data residency | OAuth2/OIDC, authz hook, safety screening, SSRF guard, signed manifests, audit `trace_id`, Vault/KMS | Identity-bound share of AI traffic, time to reconstruct an incident, questionnaire turnaround |
| [5. Customer Operations](pillars/05-customer-operations.md) | Faster, consistent answers in the tools teams already use | Internal ticket deflection; earlier churn warning; clean handovers; people stay in control | A2A agents, Slack and Teams, help-desk and knowledge MCP servers, agent analytics | First-response time, resolution time, reopen rate, deflection, net revenue retention |
| [6. AI FinOps & Platform Operations](pillars/06-ai-finops-operations.md) | AI spend attributed, capped and forecastable | Right model for the job; provider independence; resilience; production-grade operation | Gateway routing, pricing, quotas, retry, `analytics costs`, probes, Prometheus, OTLP | Attributed share of AI spend, spend vs budget, cost per workflow, availability |
| [7. Knowledge & Content](pillars/07-knowledge-content.md) | Expertise captured as owned, versioned skills that reach the right people | One current version; less duplicated work; faster content; retained know-how | Marketplaces, plugins, skills, bridge manifest sync, content and web management, usage analytics | Skills in production, users on current version, time for a change to reach users, content cycle time |

## Value framework

| Page | Use it for |
|------|-----------|
| [Value model](value-model.md) | The equation, the five bottom-line levers, value formulas, lever coverage and every value chain |
| [Connectors catalogue](connectors.md) | Every data source to connect, default access and controls, and which pillars it feeds |
| [KPI catalogue](kpis.md) | Every leading and lagging KPI by lever, with its bottom-line translation and source |

## Where to start

Pick the pillar with the clearest baseline and an executive who owns the KPI. In practice:

- If security is blocking AI adoption, start with **Governance** and make it the foundation for everything else.
- If the board is asking what AI spend buys, start with **FinOps**.
- If leadership wants to know how teams are performing, start with **People & Performance**, with **Governance** controls on people data from day one.
- If one department has a burning workflow problem, start with that department's pillar and land **Governance** controls alongside it.

## Measuring value

For every pillar in scope:

1. Baseline the primary KPIs before configuration starts.
2. Agree the measurement source (platform analytics, CRM, help desk, delivery tooling, finance).
3. Measure at 30, 60 and 90 days.
4. Record results in the customer's own terms. Sales assets quote only measured results: `[INSERT: measured result from customer deployment]`.
