# Pillar 3: Engineering Productivity

Coding agents (Claude Code, SDK-built agents, MCP hosts) running through one governed gateway, with shared engineering skills, scoped tool access to repositories and delivery systems, and a clear view of usage and cost per team.

## 1. Who it's for

- **Buyer**: CTO, VP Engineering, Head of Platform or Developer Experience.
- **Users**: software engineers, SREs, QA, engineering managers, platform teams.
- **Partner role delivering it**: consultant or developer holding `SP-SPx-ENG`, usually with an Architect for gateway and MCP design.

## 2. Business problems solved

1. Engineers use AI coding tools on personal API keys or unmanaged accounts. Nobody knows what code or credentials leave the building.
2. Each team reinvents its own prompts, rules and review checklists; standards do not travel.
3. AI spend on engineering grows with no attribution to team, repository or project.
4. Coding agents need access to issue trackers, CI, documentation and internal APIs, and that access is granted ad hoc.
5. Leaders cannot tell whether AI tooling is improving delivery or just generating more code to review.
6. Switching model provider means touching every tool's configuration.

## 3. Benefits delivered

| Benefit | Mechanism | Enabling capability | KPI | How to baseline |
|---------|-----------|---------------------|-----|-----------------|
| One governed endpoint for every coding tool | Claude Code and SDK apps point at the gateway's `/v1` surface; identity replaces shared keys | Gateway (`/v1/messages`, `/v1/chat/completions`, `/v1/responses`), bridge credential helper | Share of engineering AI traffic through the gateway; personal keys retired | Count of keys and tools in use today |
| Shared engineering standards | Coding standards, review checklists, rules and hooks ship as plugins to every engineer | Marketplace plugins, skills, rules and hooks | Review rounds per PR; standards violations found in review | Last quarter's PR data |
| Safe tool access for agents | Agents reach issue trackers, CI and internal APIs through MCP servers with scoped tools and their own OAuth and access log | Governed MCP servers, `infra logs tools` | Tool calls denied by policy; incidents from agent actions | Current access model inventory |
| Credentials stay out of prompts | Requests are screened for credential patterns and blocked terms before they reach a provider | Gateway safety screening and policy, MCP enforcement | Credential-pattern detections per month | Secret-scanning history |
| Cost attributed to teams | Every request carries identity; cost is accounted per user and rolled up | `analytics costs`, per-user cost, quotas | AI cost per engineer and per team; spend vs budget | Current invoice by account |
| Measurable delivery impact | Usage data is joined with delivery metrics | Analytics plus OTLP ingest from clients (`/v1/otel`) | Lead time for changes, deployment frequency, change failure rate, PR cycle time | DORA metrics for the prior two quarters |
| Provider choice without code changes | Model patterns route to configured upstreams; switching is configuration | Gateway routes and provider registry | Time to switch or add a provider | Current effort to change provider |

## 4. Value map

How this pillar turns connected data into movement on the bottom line. Read every row of the value chain as **data source + implementation + control = expected benefit**, then follow the benefit to its lever and KPI. Levers and value formulas are defined in the [value model](../value-model.md); every connector is described in the [connectors catalogue](../connectors.md) and every KPI in the [KPI catalogue](../kpis.md).

### 4.1 Data sources to connect

| Data source | Example systems | Data used | Access | Connected via | Data owner |
|-------------|-----------------|-----------|--------|---------------|------------|
| Code repositories | GitHub, GitLab, Bitbucket | Code, PRs, reviews, commit history | Read; write via PR only | MCP server, per-user OAuth | Engineering |
| Issue tracking | Jira, Linear | Tickets, cycle time, sprint data | Read; comment and transition on approval | MCP server | Engineering managers |
| CI/CD | GitHub Actions, GitLab CI, Jenkins, Argo | Build results, deploy history, failure logs | Read | MCP server | Platform team |
| Observability | Datadog, Grafana, Sentry, PagerDuty | Errors, alerts, incidents, traces | Read | MCP server | SRE |
| Engineering docs | Confluence, Notion, ADRs, runbooks | Standards, architecture decisions, runbooks | Read | MCP server; skills | Architecture owners |
| AI coding clients | Claude Code, SDK apps, MCP hosts | Model requests, tool calls, client telemetry | Routed through the gateway; OTLP ingest | Gateway, bridge | Platform team |
| Model providers | Anthropic, OpenAI, Gemini, internal proxy | Inference | Routed | Gateway routes | Platform team |

### 4.2 Value chain

| # | Data source (input) | + Implementation | + Control | = Expected benefit (output) | Lever |
|---|---------------------|------------------|-----------|-----------------------------|-------|
| 1 | AI coding clients + model providers | All coding tools point at the gateway; bridge handles credentials | SSO identity; personal keys retired; network blocks direct provider access | One governed, metered endpoint for all engineering AI | Cost control, Risk |
| 2 | Engineering docs + standards | Standards plugin with skills, rules and hooks distributed to every engineer | Versioned plugin; one owner; signed manifest | Fewer standards issues in review; consistent code | Efficiency, Performance |
| 3 | Repositories + issue tracker | Coding agents work tickets end to end and open PRs | Write only via PR; human review required; tool scopes per agent | Shorter cycle time; more PRs merged per engineer | Efficiency, Performance |
| 4 | Observability + deploy history + tracker | Incident triage agent drafts the first triage note | Read-only; every tool call audited under one `trace_id` | Faster time to restore service | Performance, Risk |
| 5 | All requests | Credential-pattern screening and policy on every request | Block and log; alerts to security | Secrets kept out of prompts and providers | Risk |
| 6 | Gateway usage | Cost per engineer, team and repository; quotas | Team quotas with alerting | AI spend attributed and inside budget | Cost control |

### 4.3 KPI map

| Lever | Leading KPI (input, moves first) | Lagging KPI (output, moves the P&L) | Bottom-line translation | Measured from |
|-------|----------------------------------|-------------------------------------|-------------------------|---------------|
| Efficiency | PR cycle time; review rounds per PR | Lead time for changes; features delivered per sprint | Hours saved × engineers × weeks × loaded hourly cost, or Δ throughput at constant headcount | Repo and tracker data, DORA |
| Performance | Change failure rate; mean time to restore | Production incidents; SLA breaches | Δ incident hours × cost of downtime per hour | Observability, incident records |
| Cost control | AI cost per engineer; share of traffic on the gateway | AI spend vs budget | Spend avoided by quotas and model tiering; consolidated licences | `analytics costs`, invoices |
| Risk | Credential-pattern detections; personal keys retired | Security incidents involving AI tools | Avoided incident cost | Audit log, security records |

### 4.4 Expected benefit

**Net annual value = sum of the lever values in 4.3 − (platform cost + AI usage cost + delivery cost).** Every input comes from the customer's own baseline, taken in discovery (section 7), and is re-measured at 30, 60 and 90 days. Quote results only from measured data: `[INSERT: measured result from customer deployment]`.

## 5. Platform capabilities used

- **Gateway** and its controls: quota, policy, safety screening, audit, SSRF guard. See [concepts/gateway.md](../../concepts/gateway.md) and [guides/configure-providers.md](../../guides/configure-providers.md).
- **Bridge**: credential helper, signed manifest sync of plugins, skills and MCP allowlists, and a local inference proxy for desktop clients.
- **MCP servers** for repositories, issue trackers, CI and internal services. See [concepts/mcp.md](../../concepts/mcp.md).
- **A2A agents** for longer-running engineering workflows (triage, release notes, dependency review). See [concepts/a2a-protocol.md](../../concepts/a2a-protocol.md).
- **OTLP ingest and export**, Prometheus metrics. See [guides/operate.md](../../guides/operate.md).
- **Extensions** for company-specific tools and gateway guards. See [guides/authoring-extensions.md](../../guides/authoring-extensions.md).

## 6. Reference use cases

**Retire personal API keys.** Before: engineers bring their own keys; finance sees a pile of card charges. After: Claude Code authenticates through the bridge to the gateway with SSO; spend is attributed and capped per team.

**Standards as plugins.** Before: the coding standard is a wiki page. After: it ships as a plugin with skills, rules and hooks; when it changes, every engineer gets the new version on next sync.

**Incident triage agent.** Before: on-call engineers gather logs, recent deploys and tickets by hand. After: an agent with read-only MCP access to observability, deploy history and the issue tracker produces a first triage note; every tool call is in the audit log under one `trace_id`.

**Model evaluation.** Before: trying a new model means changing every tool. After: the platform team adds a route, scopes it to a pilot group and compares cost and outcomes in analytics.

## 7. Implementation pattern

1. **Discover**: inventory AI tools, keys and accounts in engineering; baseline DORA and PR metrics; list the systems agents need.
2. **Configure**: gateway routes and providers; SSO; bridge enrolment; engineering plugin set; MCP servers with read-only scopes first; quotas per team.
3. **Pilot**: one or two squads; migrate their tools to the gateway; tune policy and safety screening against false positives.
4. **Roll out**: all teams; retire personal keys; publish standards plugins.
5. **Measure**: monthly cost per team, quarterly DORA comparison.

Typical deliverables: tool and key inventory, gateway and route configuration, engineering plugin set, MCP access register, cost and delivery dashboard.

## 8. Risks and governance guardrails

- **Scope of enforcement**: the gateway governs model traffic routed through it, and MCP enforcement governs tools served by governed MCP servers. Tools that execute locally on a laptop outside those paths are not governed by the platform. Say so in design reviews.
- **Write access**: start agents read-only against repositories and production systems. Grant write tools per agent with a named owner.
- **Quota semantics**: cost is accounted after completion, so concurrent requests can exceed a ceiling. Set quotas with headroom and alert before the limit.
- **Measuring output, not volume**: lines of code generated is not a productivity measure. Use delivery and quality metrics.

## 9. Certification objectives (`SP-SPx-ENG`)

Candidates can:
- Point Claude Code and an SDK application at the gateway and explain the authentication path through the bridge.
- Package an engineering standard as a plugin with skills, rules and hooks, and distribute it.
- Design MCP access for an agent with least-privilege tool scopes.
- Configure team quotas and read cost per team.
- Explain precisely which engineering activity the platform governs and which it does not.
- Build the value map for a customer: data sources to connect, implementation, controls, levers, and the value formula filled with the customer's own baseline.

Lab: route Claude Code through a demo gateway, enrol via the bridge, install a standards plugin, trigger a credential-pattern block, then show the request, the block and the cost in the audit and analytics output.
