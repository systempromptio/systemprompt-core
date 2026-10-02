# Pillar 1: Revenue & Growth

Sales and marketing teams running the same well-built skills against live CRM, calendar and content data, so the quality of a call brief or a campaign draft stops depending on which rep or marketer wrote the prompt.

## 1. Who it's for

- **Buyer**: CRO, VP Sales, CMO, Head of Revenue Operations.
- **Users**: account executives, SDRs, account managers, customer success managers, marketers, RevOps analysts.
- **Partner role delivering it**: functional consultant holding `SP-SPx-REV`, with an Administrator for the connector and identity setup.

## 2. Business problems solved

1. Every rep uses AI differently. The best reps have great prompts; the rest paste CRM exports into a chat window and get generic output.
2. Call preparation, follow-up emails and CRM updates eat selling time, and CRM hygiene degrades because nobody wants to do the data entry.
3. Pipeline reviews run on stale fields. Risk (quiet deals, slipping close dates, missing stakeholders) is found late.
4. Marketing content drifts off brand and off message as more people generate it.
5. Leadership cannot see which AI workflows are used, by whom, or whether they correlate with outcomes.
6. CRM data leaves the company through unmanaged AI tools.

## 3. Benefits delivered

| Benefit | Mechanism | Enabling capability | KPI | How to baseline |
|---------|-----------|---------------------|-----|-----------------|
| Consistent sales execution | The organisation's best call-prep, deal-review and follow-up methods are written once as skills and distributed to every rep | Marketplace plugins and skills scoped by role | Variance in activity quality across reps; manager-scored call-prep quality | Score a sample of current call briefs before rollout |
| More selling time | Skills assemble briefs and draft follow-ups from CRM and calendar data through MCP connectors, instead of reps copying data by hand | Governed MCP servers to CRM, email, calendar | Hours per rep per week on prep and admin; meetings per rep per week | Time study or self-reported diary for two weeks |
| Earlier deal-risk detection | Scheduled and on-demand skills scan pipeline for quiet deals, overdue close dates and single-threaded opportunities | Skills plus scheduled jobs; MCP read access to CRM | Forecast accuracy; slipped-deal rate; days from risk signal to action | Last two quarters' forecast vs actual |
| Better CRM hygiene | Follow-up skills propose structured CRM updates the rep approves | MCP write tools, audited per call | Field completeness on open opportunities; next-step freshness | Hygiene check on current pipeline |
| On-brand content at volume | Brand voice and campaign skills encode tone, banned phrasing and positioning | Skills distributed through the marketplace, versioned | Content pieces shipped per marketer; review rounds per piece | Count of rounds on the last 20 assets |
| Visibility of what works | Skill and tool usage by user and team is recorded | Analytics: `analytics tools`, `analytics requests`, usage by user | Weekly active users per skill; skill usage vs pipeline outcomes | Zero (no visibility today) |
| CRM data stays governed | Requests are identity-bound and tool calls are audited; access is scoped per role | OAuth/OIDC, MCP scoping, audit with `trace_id` | Unmanaged AI tools in use by sales; audit coverage of CRM access by AI | Shadow-AI survey or proxy logs |

## 4. Value map

How this pillar turns connected data into movement on the bottom line. Read every row of the value chain as **data source + implementation + control = expected benefit**, then follow the benefit to its lever and KPI. Levers and value formulas are defined in the [value model](../value-model.md); every connector is described in the [connectors catalogue](../connectors.md) and every KPI in the [KPI catalogue](../kpis.md).

### 4.1 Data sources to connect

| Data source | Example systems | Data used | Access | Connected via | Data owner |
|-------------|-----------------|-----------|--------|---------------|------------|
| CRM | Salesforce, HubSpot, Microsoft Dynamics | Accounts, opportunities, contacts, activities, forecast fields | Read; write to approved fields only | MCP server, per-user OAuth | Revenue Operations |
| Email and calendar | Google Workspace, Microsoft 365 | Meetings, attendees, customer threads | Read | MCP server, per-user OAuth | IT |
| Call recordings and transcripts | Gong, Zoom, Microsoft Teams | Transcripts, talk tracks, next steps | Read | MCP server | Sales enablement |
| Marketing automation | HubSpot, Marketo, Pardot | Campaigns, lead engagement, attribution | Read | MCP server | Marketing operations |
| Brand and content | CMS, DAM, brand guidelines, messaging docs | Voice, positioning, approved claims | Read | Skills (embedded) and MCP to the DAM | Brand owner |
| Product usage and billing (optional) | Product analytics, Stripe, ERP | Usage trends, renewals, invoices | Read | MCP server | Finance, product |

### 4.2 Value chain

| # | Data source (input) | + Implementation | + Control | = Expected benefit (output) | Lever |
|---|---------------------|------------------|-----------|-----------------------------|-------|
| 1 | CRM + calendar + email | Call-prep skill builds a one-page brief per meeting | Read-only scopes; each rep sees only their own accounts; every read audited | Every call is prepared; prep time drops | Efficiency, Revenue |
| 2 | Transcripts + CRM | Follow-up skill drafts the customer email and proposed CRM updates | Rep approves before anything is sent or written; write tools limited to named fields | Faster follow-up; complete, current CRM | Efficiency, Performance |
| 3 | CRM pipeline + activity | Scheduled deal-signals job flags quiet deals, slipping dates, single-threaded opportunities | Digest goes to the deal owner and their manager only | Risk found weeks earlier; more accurate forecast | Revenue, Performance |
| 4 | CRM + product usage + billing | Expansion and renewal skill surfaces upsell and churn signals | Scoped to the account owner; no customer contact without a person | More expansion; fewer surprise churns | Revenue |
| 5 | Brand guidelines + marketing automation | Campaign, email-sequence and brand-review skills | Skills versioned and owned by the brand lead; safety screening on every request | More on-brand content per marketer; fewer review rounds; less agency spend | Efficiency, Cost control |
| 6 | All of the above, through the gateway | Usage and cost analytics per team | Team quotas; data stays on governed paths | Known AI cost per rep; CRM data off unmanaged tools | Cost control, Risk |

### 4.3 KPI map

| Lever | Leading KPI (input, moves first) | Lagging KPI (output, moves the P&L) | Bottom-line translation | Measured from |
|-------|----------------------------------|-------------------------------------|-------------------------|---------------|
| Efficiency | Prep and admin hours per rep per week | Share of time spent selling | Hours saved × reps × working weeks × loaded hourly cost | Time study, `analytics tools` |
| Revenue | Follow-up sent within 24 hours; stage-to-stage conversion | Win rate; average deal size; sales cycle length | Δ win rate × qualified pipeline value; Δ cycle length × pipeline value × cost of capital | CRM |
| Performance | Forecast submitted with evidence; next-step freshness | Forecast accuracy (forecast vs actual) | Fewer missed quarters; less over-hiring or under-hiring against a wrong forecast | CRM, finance |
| Cost control | Agency and freelance content spend; AI cost per rep | Cost of sale | Δ agency spend + Δ AI spend against budget | Finance, `analytics costs` |
| Risk | Share of CRM access by AI that is audited | Data incidents involving customer data | Avoided incident cost (customer's own incident cost model) | Audit log |

### 4.4 Expected benefit

**Net annual value = sum of the lever values in 4.3 − (platform cost + AI usage cost + delivery cost).** Every input comes from the customer's own baseline, taken in discovery (section 7), and is re-measured at 30, 60 and 90 days. Quote results only from measured data: `[INSERT: measured result from customer deployment]`.

## 5. Platform capabilities used

- **Skills and plugins** distributed from a marketplace, scoped to sales or marketing roles. See [guides/marketplace-authoring.md](../../guides/marketplace-authoring.md).
- **MCP servers** for CRM, email, calendar and document stores, each with its own scoped tool exposure, OAuth and access log. See [concepts/mcp.md](../../concepts/mcp.md).
- **Gateway** so Claude Cowork, Claude Code and SDK apps all reach models through one metered, audited endpoint. See [concepts/gateway.md](../../concepts/gateway.md).
- **Bridge** to put the signed plugin and MCP allowlist on each rep's desktop.
- **Analytics** for tool, request and per-user usage. See [reference/cli.md](../../reference/cli.md) (`analytics`).

## 6. Reference use cases

**Call preparation.** Before: the rep opens the CRM, last emails and LinkedIn, and assembles context by hand, if they do it at all. After: the rep runs a call-prep skill that pulls account, opportunity, contacts and recent activity through the CRM MCP server and returns a one-page brief with open threads and discovery gaps.

**Post-call follow-up.** Before: notes live in the rep's head; the CRM is updated days later. After: a follow-up skill drafts the customer email and an internal recap from the transcript, and proposes CRM field changes the rep approves. Each write is an audited tool call.

**Weekly pipeline risk digest.** Before: managers inspect deals one by one in the forecast call. After: a deal-signals skill produces a risk digest per team (quiet deals, close-date danger, competitor mentions) ahead of the call.

**Campaign production.** Before: each marketer prompts from scratch; brand review catches problems late. After: campaign, email-sequence and brand-review skills share one voice definition maintained by the brand owner.

## 7. Implementation pattern

1. **Discover** (1 to 2 weeks): map the sales motion, pick three workflows with the clearest time cost, baseline the KPIs above, inventory the CRM and data sources.
2. **Configure**: connect CRM, email and calendar MCP servers with least-privilege scopes; author or import the sales and marketing plugins; scope them to roles; set quotas.
3. **Pilot** (4 to 6 weeks): one team, with the manager as sponsor. Review skill output quality weekly and revise skills, not individual prompts.
4. **Roll out**: extend to all teams; publish skill versions through the marketplace.
5. **Measure**: compare KPIs to baseline at 30, 60 and 90 days using analytics plus CRM data.

Typical deliverables: discovery readout, connector and scope register, sales and marketing plugin set, role-to-plugin mapping, KPI dashboard, 90-day value review.

## 8. Risks and governance guardrails

- **Write access to CRM**: start read-only; enable write tools only for approved skills, and keep a human approval step for field changes.
- **Customer PII in prompts**: route all traffic through the gateway so safety screening and audit apply; restrict which models CRM data may reach.
- **Outbound messaging**: skills draft, people send. Do not give an agent unsupervised send rights to customers.
- **Skill sprawl**: one owner per skill, versioned in the marketplace; retire duplicates.

## 9. Certification objectives (`SP-SPx-REV`)

Candidates can:
- Run discovery with a sales leader and identify the three highest-value workflows with measurable baselines.
- Design a sales and marketing plugin set and map it to roles.
- Configure CRM, email and calendar MCP servers with least-privilege scopes and explain each scope.
- Explain where human approval sits for CRM writes and outbound communication.
- Produce a usage and outcome readout from analytics and CRM data.
- Build the value map for a customer: data sources to connect, implementation, controls, levers, and the value formula filled with the customer's own baseline.

Lab: on a demo tenant with a sandbox CRM, import a sales plugin, scope it to an "AE" role, run a call-prep and a follow-up skill end to end, then show the audit trail for the CRM tool calls and the usage for that role.
