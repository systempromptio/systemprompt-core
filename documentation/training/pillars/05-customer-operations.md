# Pillar 5: Customer Operations

Agents that help support and customer success teams answer faster and more consistently, working inside Slack and Microsoft Teams, grounded in governed knowledge and tools, with people kept in charge of what reaches the customer.

## 1. Who it's for

- **Buyer**: VP Customer Support, VP Customer Success, COO, Head of Service Operations.
- **Users**: support agents, CSMs, account managers, internal service desks (IT, HR, finance help desks).
- **Partner role delivering it**: consultant holding `SP-SPx-CXO`, with an Administrator for Slack, Teams and MCP connections.

## 2. Business problems solved

1. Answers depend on which agent picks up the case; knowledge sits in people's heads and scattered documents.
2. Case summaries, handovers and escalation notes are slow and inconsistent.
3. Customer health signals are spread across CRM, cases, usage and meetings; at-risk accounts are found late.
4. Internal service desks answer the same questions repeatedly.
5. Teams want AI help in the tools they already work in (Slack, Teams), not in another tab.
6. Leadership worries about AI saying the wrong thing to a customer.

## 3. Benefits delivered

| Benefit | Mechanism | Enabling capability | KPI | How to baseline |
|---------|-----------|---------------------|-----|-----------------|
| Faster first response | Agents draft answers from governed knowledge and case history for a person to review | A2A agents, MCP servers to the help desk and knowledge base | First-response time; time to resolution | Help-desk reports for the last quarter |
| Consistent answers | The same skills and sources serve every agent and every channel | Skills and plugins via marketplace; versioned content | Reopen rate; QA score variance across agents | QA sample before rollout |
| AI where people work | Agents and skills are reachable from Slack and Microsoft Teams | Slack and Teams domains (events, Block Kit, adaptive cards) | Share of AI interactions from chat tools; adoption in support teams | Current tool usage |
| Internal ticket deflection | An internal service-desk agent answers routine IT, HR or finance questions from approved sources | A2A agents in Slack or Teams, scoped knowledge | Tickets deflected; tickets per employee | Service-desk volume by category |
| Earlier churn warning | Health skills combine CRM, case and activity data into a scored view | Skills, MCP read access, scheduled jobs | Gross and net revenue retention; renewals flagged at risk early enough to act | Renewal outcomes last year |
| Clean handovers | Summarisation skills produce structured escalation and handover notes | Skills, A2A tasks and artifacts | Escalation handling time; handover defects | Sample of current escalations |
| Control over what customers see | Agents draft, people send; every tool call and response is audited | Audit with `trace_id`, policy on tools | Customer-facing AI errors | Zero baseline expected; track from day one |

## 4. Value map

How this pillar turns connected data into movement on the bottom line. Read every row of the value chain as **data source + implementation + control = expected benefit**, then follow the benefit to its lever and KPI. Levers and value formulas are defined in the [value model](../value-model.md); every connector is described in the [connectors catalogue](../connectors.md) and every KPI in the [KPI catalogue](../kpis.md).

### 4.1 Data sources to connect

| Data source | Example systems | Data used | Access | Connected via | Data owner |
|-------------|-----------------|-----------|--------|---------------|------------|
| Help desk | Zendesk, ServiceNow, Freshdesk, Salesforce Service Cloud | Cases, history, macros, SLAs | Read; draft replies and internal notes | MCP server, per-user OAuth | Support operations |
| Knowledge base | Confluence, Guru, Zendesk Guide, SharePoint | Articles, policies, troubleshooting guides | Read | MCP server | Knowledge owners |
| CRM | Salesforce, HubSpot | Accounts, renewals, contacts, health fields | Read | MCP server | Customer success operations |
| Product usage | Product analytics, telemetry | Usage trends, feature adoption | Read | MCP server | Product |
| Chat platforms | Slack, Microsoft Teams | Channel and direct messages to agents | Read and reply in scoped channels | Slack and Teams domains | IT |
| Internal service desks | ServiceNow, Jira Service Management, HR and IT portals | Internal tickets and policy content | Read; create tickets | MCP server | IT, HR, finance |

### 4.2 Value chain

| # | Data source (input) | + Implementation | + Control | = Expected benefit (output) | Lever |
|---|---------------------|------------------|-----------|-----------------------------|-------|
| 1 | Help desk + knowledge base | Reply-drafting agent cites articles and similar resolved cases | Agent drafts, a person sends; user sees only cases they may see | Faster first response; consistent answers | Efficiency, Performance |
| 2 | Internal service desk + knowledge in Slack or Teams | Internal help-desk agent answers routine IT, HR and finance questions | Approved sources only; opens a ticket when unsure | Routine tickets deflected | Cost control, Efficiency |
| 3 | Help desk + case history | Summarisation skill writes structured escalation and handover notes | Attached to the case; audited | Shorter escalations; fewer handover defects | Efficiency |
| 4 | CRM + cases + product usage | Customer-health skill and scheduled renewal-risk digest | Scoped to account owner and CS manager | At-risk accounts found while there is time to act | Revenue |
| 5 | All agent interactions | Agent and conversation analytics; QA sampling | Policy on tools; tone and commitment rules in skills | Quality visible; errors caught before customers do | Performance, Risk |

### 4.3 KPI map

| Lever | Leading KPI (input, moves first) | Lagging KPI (output, moves the P&L) | Bottom-line translation | Measured from |
|-------|----------------------------------|-------------------------------------|-------------------------|---------------|
| Efficiency | First-response time; handling time per case | Cases handled per agent | Hours saved × agents × weeks × loaded hourly cost, or cases absorbed without new hires | Help-desk reports |
| Cost control | Deflection rate on internal desks | Tickets per employee; service-desk headcount need | Deflected tickets × cost per ticket | Service-desk reports |
| Performance | Reopen rate; QA score | CSAT; resolution time | Linked to retention through the customer's CSAT-to-churn data | Help desk, surveys |
| Revenue | Renewals flagged at risk 90+ days out | Gross and net revenue retention | Δ retention × annual recurring revenue | CRM, finance |
| Risk | Customer-facing AI errors caught in QA | Complaints and credits issued due to wrong answers | Avoided credits and complaint handling | Help desk, finance |

### 4.4 Expected benefit

**Net annual value = sum of the lever values in 4.3 − (platform cost + AI usage cost + delivery cost).** Every input comes from the customer's own baseline, taken in discovery (section 7), and is re-measured at 30, 60 and 90 days. Quote results only from measured data: `[INSERT: measured result from customer deployment]`.

## 5. Platform capabilities used

- **A2A agents** with tasks, contexts, artifacts and streaming. See [concepts/a2a-protocol.md](../../concepts/a2a-protocol.md).
- **Slack and Microsoft Teams** integration domains for chat-native agents.
- **MCP servers** for help desk, CRM, knowledge base and product usage data. See [concepts/mcp.md](../../concepts/mcp.md).
- **Skills and plugins** for summaries, health scoring and response drafting.
- **Analytics** for agent performance and conversations (`analytics agents`, `analytics conversations`).

## 6. Reference use cases

**Support reply drafting.** Before: the agent searches three systems and writes from scratch. After: in the help-desk view or Slack, an agent proposes a reply citing the knowledge article and similar resolved cases; the support agent edits and sends.

**Internal IT help desk in Teams.** Before: employees open tickets for password, access and policy questions. After: a Teams agent answers from the approved IT knowledge base and opens a ticket when it cannot.

**Renewal risk review.** Before: CSMs prepare QBRs by hand and spot risk late. After: a customer-health skill pulls renewals, cases and activity and flags accounts that need a plan this month.

**Escalation handover.** Before: escalations arrive with a thread to read. After: a summarisation skill produces a structured note (issue, impact, steps tried, customer sentiment, ask) attached to the case.

## 7. Implementation pattern

1. **Discover**: case categories by volume; knowledge sources and their owners; baseline FRT, resolution time, reopen rate, deflection.
2. **Configure**: knowledge and help-desk MCP servers (read first); agents and their tool scopes; Slack or Teams apps; response-drafting and summarisation skills.
3. **Pilot**: one queue or one internal service desk; human review on every response.
4. **Roll out**: more queues; introduce deflection for internal desks first, external customers later.
5. **Measure**: weekly service KPIs; monthly agent performance and conversation analytics.

Typical deliverables: knowledge source register, agent definitions, Slack or Teams apps, skill set, QA rubric, service dashboard.

## 8. Risks and governance guardrails

- **Human in the loop for customers**: external answers are drafts until a person sends them, at least until quality is proven per category.
- **Knowledge freshness**: an agent is only as good as its sources. Assign owners and review dates to every source.
- **Customer data scoping**: agents only see the accounts and cases the requesting user may see.
- **Tone and commitments**: skills must not promise refunds, credits or timelines outside policy. Encode the policy in the skill and test it.

## 9. Certification objectives (`SP-SPx-CXO`)

Candidates can:
- Prioritise support and service-desk use cases by volume and risk.
- Define an A2A agent with scoped MCP tools and deploy it to Slack or Teams.
- Design a human-review flow for customer-facing responses.
- Measure service KPIs and agent performance against baseline.
- Explain how customer data access is scoped per requesting user.
- Build the value map for a customer: data sources to connect, implementation, controls, levers, and the value formula filled with the customer's own baseline.

Lab: deploy an internal help-desk agent to a demo Slack or Teams workspace, ground it in a sample knowledge base through an MCP server, answer three seeded questions, escalate a fourth, and show the agent and conversation analytics afterwards.
