# Connectors catalogue

Every data source a pillar depends on, what the platform takes from it, how it connects, the control that applies by default, and which pillars it feeds. Connect a source once and every pillar that uses it benefits.

Example systems are the products customers most often run in each category. The platform does not ship a connector for every product. A business system connects through an MCP server, either an existing one or one the partner builds or compiles in as an extension (see [concepts/mcp.md](../concepts/mcp.md) and [guides/authoring-extensions.md](../guides/authoring-extensions.md)). Identity, secrets, providers, chat platforms and telemetry use the platform's own interfaces.

Pillar codes: **REV** Revenue & Growth, **PPL** People & Performance, **ENG** Engineering Productivity, **GOV** Governance, Security & Compliance, **CXO** Customer Operations, **FIN** AI FinOps & Platform Operations, **KNW** Knowledge & Content.

## Identity and security

| Connector | Example systems | Data used | Default access | Connected via | Default control | Pillars |
|-----------|-----------------|-----------|----------------|---------------|-----------------|---------|
| Identity provider | Okta, Entra ID, Google Workspace, Ping | Users, groups, roles, reporting lines, team and cost-centre claims | Read | OIDC SSO | Default-deny authorisation hook; roles mapped from IdP groups | All |
| Secrets and key management | HashiCorp Vault, OpenBao, AWS/GCP/Azure KMS, sops | Provider keys, signing keys | Read at boot | Profile secrets source | Master key stays in the customer's KMS; never enters the binary | GOV, FIN |
| SIEM and log platform | Splunk, Microsoft Sentinel, Elastic, Datadog | Receives audit events, logs and traces | Write (export) | Structured logs, OTLP export | Append-only audit correlated by `trace_id`; retention set to policy | GOV, ENG, FIN, PPL |
| Network and egress controls | Firewall, forward proxy, CASB | Direct provider traffic to block | Configured alongside | Network policy | Only the gateway may reach model providers | GOV |
| Data classification and DLP policy | DLP rules, data catalogue, acceptable-use policy | Restricted data classes, blocked patterns | Encoded as policy | Gateway policy, blocklists, request guards | Block, log and alert | GOV |

## AI and platform

| Connector | Example systems | Data used | Default access | Connected via | Default control | Pillars |
|-----------|-----------------|-----------|----------------|---------------|-----------------|---------|
| Model providers | Anthropic, OpenAI, Gemini, Azure OpenAI, internal proxy | Inference | Routed | Gateway routes and provider registry | Provider allowlist, quotas, safety screening, audit, SSRF guard | All |
| AI clients | Claude Code, Claude Cowork, SDK applications, MCP hosts | Model requests, tool calls, client telemetry | Routed through the gateway; OTLP ingest | Gateway `/v1`, bridge | SSO identity per request; signed plugin and MCP allowlist manifest | All |
| Marketplace repositories | Git repositories holding plugins, skills, agents, rules and hooks | Versioned skills and plugins | Read; authors write | Marketplace import | Owner and version per plugin; authorisation cascade decides who sees each entry | REV, PPL, ENG, CXO, KNW |
| Platform usage analytics | Built in | Requests, tools, agents, sessions, conversations, costs | Read | `analytics` commands | Access limited to admin roles | All |
| Monitoring stack | Prometheus, Grafana, Datadog, PagerDuty | Health, metrics, alerts | Scrape and export | Metrics endpoint, liveness and readiness probes, OTLP | Alert thresholds; change control on upgrades | FIN, ENG |
| Finance and BI | Provider invoices, ERP, cost-centre structure, BI tool | Budgets, prices, chargeback targets | Read for reconciliation; receives exports | Finance process, analytics and OTLP export | Finance sign-off on chargeback | FIN, PPL |

## Revenue and customer systems

| Connector | Example systems | Data used | Default access | Connected via | Default control | Pillars |
|-----------|-----------------|-----------|----------------|---------------|-----------------|---------|
| CRM | Salesforce, HubSpot, Microsoft Dynamics | Accounts, opportunities, contacts, activities, forecast fields, renewals | Read; write to approved fields only | MCP server, per-user OAuth | User sees only records the CRM grants them; a person approves every write | REV, PPL, CXO |
| Marketing automation | HubSpot, Marketo, Pardot | Campaigns, lead engagement, attribution | Read | MCP server | Read-only | REV |
| Call recordings and transcripts | Gong, Zoom, Microsoft Teams | Transcripts, next steps | Read | MCP server | Respect the recording-consent policy; scoped to the deal owner | REV |
| Help desk | Zendesk, ServiceNow, Freshdesk, Salesforce Service Cloud | Cases, history, macros, SLAs | Read; draft replies and internal notes | MCP server, per-user OAuth | Agent drafts, a person sends | CXO, PPL |
| Internal service desks | ServiceNow, Jira Service Management, HR and IT portals | Internal tickets, policy content | Read; create tickets | MCP server | Answers only from approved sources; escalates when unsure | CXO |
| Product usage and billing | Product analytics, Stripe, ERP | Usage trends, renewals, invoices | Read | MCP server | Scoped to account owner | REV, CXO |

## People systems

| Connector | Example systems | Data used | Default access | Connected via | Default control | Pillars |
|-----------|-----------------|-----------|----------------|---------------|-----------------|---------|
| HRIS | Workday, BambooHR, HiBob, SAP SuccessFactors | Headcount, reporting lines, roles, tenure, leave | Read, restricted | MCP server, per-user authorisation | Fewest skills and roles possible; manager reaches only their reports; HRIS reads reviewed monthly | PPL |
| Performance and goals | Lattice, Culture Amp, 15Five, OKR sheets | Goals, review cycles, ratings history, feedback | Read; write draft goals only | MCP server | People make every rating decision; output marked as evidence | PPL |

## Work systems

| Connector | Example systems | Data used | Default access | Connected via | Default control | Pillars |
|-----------|-----------------|-----------|----------------|---------------|-----------------|---------|
| Project and issue tracking | Jira, Linear, Asana, Monday | Work items, cycle time, work in progress, sprints | Read; comment and transition on approval | MCP server | Team-level aggregation for productivity views | PPL, ENG |
| Code repositories | GitHub, GitLab, Bitbucket | Code, PRs, reviews, commit history | Read; write via PR only | MCP server, per-user OAuth | Human review before merge; tool scopes per agent | PPL, ENG |
| CI/CD | GitHub Actions, GitLab CI, Jenkins, Argo | Build results, deploy history, failure logs | Read | MCP server | Read-only | ENG |
| Observability and incidents | Datadog, Grafana, Sentry, PagerDuty | Errors, alerts, incidents, traces | Read | MCP server | Read-only; every tool call audited | ENG, FIN |

## Collaboration and knowledge

| Connector | Example systems | Data used | Default access | Connected via | Default control | Pillars |
|-----------|-----------------|-----------|----------------|---------------|-----------------|---------|
| Email and calendar | Google Workspace, Microsoft 365 | Meetings, attendees, threads; meeting load and focus time | Read; aggregated for people analytics | MCP server, per-user OAuth | Per-user scope; team-level aggregation for PPL | REV, PPL |
| Chat platforms | Slack, Microsoft Teams | Messages addressed to agents | Read and reply in scoped channels | Slack and Teams domains | Agent only in channels it is added to | CXO, KNW |
| Knowledge stores | Confluence, SharePoint, Notion, Google Drive, Guru | Articles, runbooks, standards, playbooks | Read | MCP server | Sources have owners and review dates | REV, ENG, CXO, KNW |
| Brand, content and web | CMS, DAM, brand guidelines, the platform's content and web management | Voice, approved claims, pages, templates | Read; write drafts | Skills, MCP server, content domain | Brand and legal review built into the skill | REV, KNW |

## Connecting a source: the standard procedure

| Step | What the partner does | Evidence produced |
|------|-----------------------|-------------------|
| 1. Owner | Name the data owner and get written approval for the data used | Entry in the connector register |
| 2. Scope | List the tools the MCP server exposes; start read-only | Tool scope list per connector |
| 3. Identity | Use per-user OAuth or per-user authorisation so each request carries the requester's rights | Test showing a second user is denied |
| 4. Controls | Apply the default control from this catalogue; add write tools only with a named owner and a human approval step | Policy configuration |
| 5. Audit | Confirm tool calls appear in `infra logs tools` with the right identity and `trace_id` | Audit sample |
| 6. Review | Re-review scopes and audit samples quarterly (monthly for HRIS) | Review record |
