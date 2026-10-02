# Value model

How the programme connects a customer's data to the bottom line. Every pillar is built from the same equation:

**Data source + implementation + control = expected benefit → lever → KPI → value**

| Term | Meaning | Where it is defined |
|------|---------|---------------------|
| Data source | A system connected to the platform, the input | [Connectors catalogue](connectors.md) |
| Implementation | The skill, agent, scheduled job or gateway configuration that uses the data | Pillar section 4.2 |
| Control | The governance rule that makes it safe: scope, identity, approval, quota, audit | Pillar section 4.2, [connectors catalogue](connectors.md) |
| Expected benefit | What changes in the business, the output | Pillar sections 3 and 4.2 |
| Lever | Which of five bottom-line levers the benefit moves | Below |
| KPI | The leading and lagging measures of that lever | [KPI catalogue](kpis.md) |
| Value | The lever formula applied to measured KPI movement | Below |

## Bottom-line levers

| Lever | What moves on the P&L | Formula | Inputs the customer supplies |
|-------|-----------------------|---------|------------------------------|
| **Revenue** | More won, expanded or retained revenue, sooner. | Δ conversion or win rate × qualified pipeline value; Δ retention × annual recurring revenue | CRM history; finance ARR |
| **Performance** | Better quality and reliability of the work itself. | Δ output (throughput, attainment, incidents) × value or cost per unit | Source-system history; customer's unit value |
| **Efficiency** | The same work in fewer hours, or more work at the same headcount. | Hours saved per person per week × people × working weeks × loaded hourly cost | Time study before and after; loaded cost from finance |
| **Cost control** | Lower or more predictable external spend. | Baseline external spend − post-rollout spend (licences, agencies, overspend, avoided hires, attrition) | Invoices, HRIS, replacement cost model |
| **Risk** | Lower probability or cost of a damaging event. | Δ incident frequency × average incident cost | Incident history; customer's incident cost model |

**Net annual value = sum of lever values − (platform cost + AI usage cost + delivery cost).** No input is a vendor benchmark; every figure comes from the customer's baseline.

## Lever coverage by pillar

● primary lever, ○ secondary lever.

| Pillar | Revenue | Performance | Efficiency | Cost control | Risk |
|--------|---------|-------------|------------|--------------|------|
| [REV](pillars/01-revenue-growth.md) Revenue & Growth | ● | ○ | ● | ○ | ○ |
| [PPL](pillars/02-people-performance.md) People & Performance | | ● | ● | ● | ○ |
| [ENG](pillars/03-engineering-productivity.md) Engineering Productivity | | ○ | ● | ● | ● |
| [GOV](pillars/04-governance-security.md) Governance, Security & Compliance | ○ | | ○ | ○ | ● |
| [CXO](pillars/05-customer-operations.md) Customer Operations | ● | ○ | ● | ● | ○ |
| [FIN](pillars/06-ai-finops-operations.md) AI FinOps & Platform Operations | | ○ | ○ | ● | ○ |
| [KNW](pillars/07-knowledge-content.md) Knowledge & Content | | ● | ● | ○ | ○ |

## Value chains by lever

Every value chain from the seven pillars, grouped by the first lever it moves.

### Revenue

| Pillar | Data source (input) | + Implementation | + Control | = Expected benefit | Levers |
|--------|---------------------|------------------|-----------|--------------------|--------|
| [REV](pillars/01-revenue-growth.md) | CRM pipeline + activity | Scheduled deal-signals job flags quiet deals, slipping dates, single-threaded opportunities | Digest goes to the deal owner and their manager only | Risk found weeks earlier; more accurate forecast | Revenue, Performance |
| [REV](pillars/01-revenue-growth.md) | CRM + product usage + billing | Expansion and renewal skill surfaces upsell and churn signals | Scoped to the account owner; no customer contact without a person | More expansion; fewer surprise churns | Revenue |
| [CXO](pillars/05-customer-operations.md) | CRM + cases + product usage | Customer-health skill and scheduled renewal-risk digest | Scoped to account owner and CS manager | At-risk accounts found while there is time to act | Revenue |

### Performance

| Pillar | Data source (input) | + Implementation | + Control | = Expected benefit | Levers |
|--------|---------------------|------------------|-----------|--------------------|--------|
| [PPL](pillars/02-people-performance.md) | Goals + work tracking | Scheduled goal-tracking job links goals to work items and flags drift weekly | Goal owner and manager only | Goals current all quarter; risk flagged before quarter end | Performance |
| [PPL](pillars/02-people-performance.md) | Work tracking + calendar + notes | 1:1 prep skill summarises recent work, blockers and open commitments | Visible to the manager and the report | Better 1:1s; faster unblocking | Performance |
| [PPL](pillars/02-people-performance.md) | Ratings history + evidence packs | Calibration pack with distribution checks | Restricted scope; fairness review each cycle | Faster, fairer calibration | Performance, Risk |
| [ENG](pillars/03-engineering-productivity.md) | Observability + deploy history + tracker | Incident triage agent drafts the first triage note | Read-only; every tool call audited under one `trace_id` | Faster time to restore service | Performance, Risk |
| [CXO](pillars/05-customer-operations.md) | All agent interactions | Agent and conversation analytics; QA sampling | Policy on tools; tone and commitment rules in skills | Quality visible; errors caught before customers do | Performance, Risk |
| [FIN](pillars/06-ai-finops-operations.md) | Monitoring stack | Probes, metrics, logs, OTLP; upgrade and rollback runbook | Alerting thresholds; change control | AI platform run like production | Performance, Risk |
| [KNW](pillars/07-knowledge-content.md) | Playbooks and SOPs | Experts and consultants convert top workflows into tested skills | Each skill has an owner, a review date and test inputs | Expertise used by everyone, not only the expert | Performance, Efficiency |
| [KNW](pillars/07-knowledge-content.md) | Marketplace versioning | A process change is a new plugin version | Retire old versions centrally | Process changes reach everyone in days, not months | Performance, Risk |

### Efficiency

| Pillar | Data source (input) | + Implementation | + Control | = Expected benefit | Levers |
|--------|---------------------|------------------|-----------|--------------------|--------|
| [REV](pillars/01-revenue-growth.md) | CRM + calendar + email | Call-prep skill builds a one-page brief per meeting | Read-only scopes; each rep sees only their own accounts; every read audited | Every call is prepared; prep time drops | Efficiency, Revenue |
| [REV](pillars/01-revenue-growth.md) | Transcripts + CRM | Follow-up skill drafts the customer email and proposed CRM updates | Rep approves before anything is sent or written; write tools limited to named fields | Faster follow-up; complete, current CRM | Efficiency, Performance |
| [REV](pillars/01-revenue-growth.md) | Brand guidelines + marketing automation | Campaign, email-sequence and brand-review skills | Skills versioned and owned by the brand lead; safety screening on every request | More on-brand content per marketer; fewer review rounds; less agency spend | Efficiency, Cost control |
| [PPL](pillars/02-people-performance.md) | Work tracking + code + CRM + help desk | Review-prep skill builds a period-wide, source-linked evidence pack per report | Manager reaches only their own reports; every read audited; output marked as evidence, not a rating | Reviews written in hours, not days; less recency bias; fewer disputes | Efficiency, Performance |
| [PPL](pillars/02-people-performance.md) | Work tracking + calendar (aggregated) | Team productivity view: throughput, cycle time, WIP, meeting vs focus hours | Team level by default; no keystroke or screen sources | Bottlenecks and meeting overload removed | Efficiency, Performance |
| [ENG](pillars/03-engineering-productivity.md) | Engineering docs + standards | Standards plugin with skills, rules and hooks distributed to every engineer | Versioned plugin; one owner; signed manifest | Fewer standards issues in review; consistent code | Efficiency, Performance |
| [ENG](pillars/03-engineering-productivity.md) | Repositories + issue tracker | Coding agents work tickets end to end and open PRs | Write only via PR; human review required; tool scopes per agent | Shorter cycle time; more PRs merged per engineer | Efficiency, Performance |
| [GOV](pillars/04-governance-security.md) | GRC tooling + audit exports | Questionnaires answered from the control matrix with live evidence | Reviewed by GRC; no certification over-claims | Faster security reviews; deals unblocked | Efficiency, Revenue |
| [CXO](pillars/05-customer-operations.md) | Help desk + knowledge base | Reply-drafting agent cites articles and similar resolved cases | Agent drafts, a person sends; user sees only cases they may see | Faster first response; consistent answers | Efficiency, Performance |
| [CXO](pillars/05-customer-operations.md) | Help desk + case history | Summarisation skill writes structured escalation and handover notes | Attached to the case; audited | Shorter escalations; fewer handover defects | Efficiency |
| [KNW](pillars/07-knowledge-content.md) | Marketplace repositories + IdP roles | Plugins scoped to roles and synced to desktops by the bridge | Signed manifest; authorisation decides who sees what | Right method in front of the right role | Efficiency |
| [KNW](pillars/07-knowledge-content.md) | Knowledge stores | Agents answer "how do we do X here" from approved sources | Read-only; sources owned and dated | Fewer interruptions of experts; faster onboarding | Efficiency |
| [KNW](pillars/07-knowledge-content.md) | Web and content | Drafting and review skills on managed content types and templates | Brand and legal review built into the skill | Faster content cycle; fewer review rounds | Efficiency, Cost control |

### Cost control

| Pillar | Data source (input) | + Implementation | + Control | = Expected benefit | Levers |
|--------|---------------------|------------------|-----------|--------------------|--------|
| [REV](pillars/01-revenue-growth.md) | All of the above, through the gateway | Usage and cost analytics per team | Team quotas; data stays on governed paths | Known AI cost per rep; CRM data off unmanaged tools | Cost control, Risk |
| [PPL](pillars/02-people-performance.md) | Calendar + work tracking + HRIS leave | Monthly overload digest to HR business partners | Team level; named HR roles only | Burnout caught early; regretted attrition falls | Cost control, Performance |
| [PPL](pillars/02-people-performance.md) | HRIS + backlog + pipeline + case volume | Capacity-planning skill compares demand with headcount and load | Restricted to planning roles; read-only HRIS | Headcount plan grounded in demand; fewer mis-hires | Cost control |
| [ENG](pillars/03-engineering-productivity.md) | AI coding clients + model providers | All coding tools point at the gateway; bridge handles credentials | SSO identity; personal keys retired; network blocks direct provider access | One governed, metered endpoint for all engineering AI | Cost control, Risk |
| [ENG](pillars/03-engineering-productivity.md) | Gateway usage | Cost per engineer, team and repository; quotas | Team quotas with alerting | AI spend attributed and inside budget | Cost control |
| [CXO](pillars/05-customer-operations.md) | Internal service desk + knowledge in Slack or Teams | Internal help-desk agent answers routine IT, HR and finance questions | Approved sources only; opens a ticket when unsure | Routine tickets deflected | Cost control, Efficiency |
| [FIN](pillars/06-ai-finops-operations.md) | Provider accounts + IdP claims | Every request metered with identity, model, tokens and price | Requests without identity are refused | Spend attributed to team and cost centre | Cost control |
| [FIN](pillars/06-ai-finops-operations.md) | Budgets from finance | Quotas per team and per agent | Enforced at the gateway; alerts before the ceiling | No surprise invoices; runaway agents stopped | Cost control |
| [FIN](pillars/06-ai-finops-operations.md) | Model catalogue + prices | Model tiering policy routes routine work to lower-cost models | Routes in configuration; quality checked against the pillar's outcome KPIs | Lower cost per completed task | Cost control, Efficiency |
| [FIN](pillars/06-ai-finops-operations.md) | Invoices + gateway records | Monthly reconciliation and chargeback | Finance sign-off | Spend forecastable; budgets owned by teams | Cost control |
| [KNW](pillars/07-knowledge-content.md) | Usage analytics | Quarterly catalogue review; retire unused and duplicate skills | Owner sign-off | Lean catalogue; investment goes to what is used | Cost control |

### Risk

| Pillar | Data source (input) | + Implementation | + Control | = Expected benefit | Levers |
|--------|---------------------|------------------|-----------|--------------------|--------|
| [ENG](pillars/03-engineering-productivity.md) | All requests | Credential-pattern screening and policy on every request | Block and log; alerts to security | Secrets kept out of prompts and providers | Risk |
| [GOV](pillars/04-governance-security.md) | Identity provider | SSO on gateway, MCP servers and agents | Default-deny authorisation hook; roles from IdP groups | Every AI request tied to a person | Risk |
| [GOV](pillars/04-governance-security.md) | Network controls + model providers | Approved clients routed through the gateway; direct provider access blocked | Provider allowlist; SSRF guard on outbound routes | Shadow AI consolidated onto one governed path | Risk, Cost control |
| [GOV](pillars/04-governance-security.md) | Data classification + policy | Safety screening, credential-pattern detection, blocklists, custom request guards | Block, log and alert | Fewer leaks of secrets and restricted data | Risk |
| [GOV](pillars/04-governance-security.md) | All governed traffic | Audit records correlated by `trace_id`, exported to the SIEM | Append-only audit; retention set to policy | Any AI action reconstructable in minutes | Risk, Efficiency |
| [GOV](pillars/04-governance-security.md) | Secrets management | Provider and signing keys held in the customer's KMS or Vault | Master key never enters the binary | Keys under the customer's own lifecycle | Risk |
| [FIN](pillars/06-ai-finops-operations.md) | Several provider accounts | Second provider configured; routes moved by configuration | Provider allowlist | No single-provider outage stops work; stronger negotiating position | Risk, Cost control |
