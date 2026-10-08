# Pillar 2: People & Performance

Performance management and productivity analytics built on evidence from the systems where work actually happens. Governed agents and skills read the HRIS, project trackers, code repositories, CRM, help desk and calendars through MCP servers, so People Ops, managers and leadership can run reviews, track goals, spot overload and plan capacity from facts instead of memory. People make every decision. The platform assembles the evidence, applies the access rules, and records who looked at what.

## 1. Who it's for

- **Buyer**: CHRO, Chief People Officer, COO, Head of People Analytics.
- **Users**: HR business partners, people analytics teams, line managers, team leads, executives in calibration and planning, works council or employee representatives (as reviewers of the configuration).
- **Partner role delivering it**: functional consultant holding `SP-SPx-PPL`, working with the customer's HR, legal and data-protection functions, and with an Administrator for the MCP connections.

## 2. Business problems solved

1. Performance reviews rely on what a manager remembers from the last few weeks. Recency bias and the loudest voices win, and reviews take managers days to write.
2. Goals and OKRs are set in January and checked in December. Progress lives in project tools nobody reconciles against the goal sheet.
3. Leadership cannot see productivity at team level across systems: delivery in Jira, code in GitHub, deals in the CRM, cases in the help desk, time in meetings.
4. Overload and burnout are noticed after someone resigns, not while the workload is building.
5. Capacity and headcount planning is argued from opinion, because there is no shared view of where the work is going.
6. People data is sensitive. Pasting HRIS exports and review notes into unmanaged AI tools is a data-protection incident waiting to happen.

## 3. Benefits delivered

| Benefit | Mechanism | Enabling capability | KPI | How to baseline |
|---------|-----------|---------------------|-----|-----------------|
| Evidence-based reviews | A review-prep skill gathers a person's delivered work for the whole period (tickets closed, PRs merged, deals progressed, cases resolved, documents authored) and drafts a balanced evidence pack for the manager | MCP servers to project, code, CRM and help-desk systems; skills; per-user identity on every read | Manager hours per review cycle; share of reviews citing period-wide evidence; rating disputes and appeals | Time survey and dispute count from the last cycle |
| Continuous goal tracking | Goals are linked to the work items that deliver them; a scheduled skill reports progress and drift to the owner and their manager | Skills plus scheduled jobs; MCP read access to goal and project systems | Goals with a current status; goals at risk flagged before quarter end | Goal-sheet freshness at last quarter end |
| Better 1:1s and feedback | A 1:1 prep skill summarises the report's recent work, blockers and open commitments since the last meeting | Skills, MCP to tracker, calendar and notes | 1:1s held as scheduled; employee rating of 1:1 usefulness | Pulse survey before rollout |
| Team productivity insight | Team-level views of throughput, cycle time, work in progress and meeting load, joined across systems | A2A agents or skills over MCP data sources; OTLP export to the customer's BI | Throughput and cycle time per team; meeting hours vs focus hours | Two quarters of history from source systems |
| Early overload warning | Signals such as sustained out-of-hours activity, rising work in progress per person and meeting saturation are surfaced at team level to HR business partners | Scheduled skills over calendar and tracker data | Regretted attrition; sick leave; teams flagged and supported | Last year's attrition and absence data |
| Grounded capacity planning | Demand (backlog, pipeline, case volume) is compared with capacity (headcount from the HRIS, current load) per team | MCP to HRIS and work systems; planning skills | Forecast vs actual hiring need; time to agree the headcount plan | Last planning cycle |
| Faster, fairer calibration | Calibration packs show the same evidence format for every person, with distribution checks across teams and demographics where the customer lawfully holds that data | Skills; restricted MCP scopes to the HRIS | Calibration meeting time; rating distribution variance across managers | Last calibration outcomes |
| People data kept governed | Every read of a person's data is identity-bound, scoped to what the requester may see, and audited | OAuth/OIDC, per-user MCP authorisation, audit with `trace_id` | Audited share of people-data access by AI; unmanaged tools holding HR data | Shadow-AI survey of HR and managers |
| AI usage as one productivity input | How teams use governed skills and agents is available alongside the work data | `analytics overview`, `analytics tools`, per-user cost | Skill usage per role, set against the team's outcome KPIs | Zero (no visibility today) |

## 4. Value map

How this pillar turns connected data into movement on the bottom line. Read every row of the value chain as **data source + implementation + control = expected benefit**, then follow the benefit to its lever and KPI. Levers and value formulas are defined in the [value model](../value-model.md); every connector is described in the [connectors catalogue](../connectors.md) and every KPI in the [KPI catalogue](../kpis.md).

### 4.1 Data sources to connect

| Data source | Example systems | Data used | Access | Connected via | Data owner |
|-------------|-----------------|-----------|--------|---------------|------------|
| HRIS | Workday, BambooHR, HiBob, SAP SuccessFactors | Headcount, reporting lines, roles, tenure, leave | Read; restricted scope | MCP server, per-user authorisation | HR operations |
| Performance and goals | Lattice, Culture Amp, 15Five, OKR sheets | Goals, review cycles, ratings history, feedback | Read; write draft goals only | MCP server | People team |
| Project and work tracking | Jira, Linear, Asana, Monday | Items delivered, cycle time, work in progress per person | Read | MCP server | PMO, engineering |
| Code and documentation | GitHub, GitLab, Confluence, Notion | PRs merged, reviews, documents authored | Read | MCP server | Engineering, knowledge owners |
| CRM and help desk | Salesforce, HubSpot, Zendesk, ServiceNow | Deals progressed, cases resolved | Read | MCP server | RevOps, support operations |
| Calendar | Google Workspace, Microsoft 365 | Meeting load, focus time, out-of-hours activity (aggregated) | Read, aggregated | MCP server | IT |
| Platform usage | systemprompt analytics | Skill and agent usage per team | Read | Built in | Platform owner |

### 4.2 Value chain

| # | Data source (input) | + Implementation | + Control | = Expected benefit (output) | Lever |
|---|---------------------|------------------|-----------|-----------------------------|-------|
| 1 | Work tracking + code + CRM + help desk | Review-prep skill builds a period-wide, source-linked evidence pack per report | Manager reaches only their own reports; every read audited; output marked as evidence, not a rating | Reviews written in hours, not days; less recency bias; fewer disputes | Efficiency, Performance |
| 2 | Goals + work tracking | Scheduled goal-tracking job links goals to work items and flags drift weekly | Goal owner and manager only | Goals current all quarter; risk flagged before quarter end | Performance |
| 3 | Work tracking + calendar + notes | 1:1 prep skill summarises recent work, blockers and open commitments | Visible to the manager and the report | Better 1:1s; faster unblocking | Performance |
| 4 | Work tracking + calendar (aggregated) | Team productivity view: throughput, cycle time, WIP, meeting vs focus hours | Team level by default; no keystroke or screen sources | Bottlenecks and meeting overload removed | Efficiency, Performance |
| 5 | Calendar + work tracking + HRIS leave | Monthly overload digest to HR business partners | Team level; named HR roles only | Burnout caught early; regretted attrition falls | Cost control, Performance |
| 6 | HRIS + backlog + pipeline + case volume | Capacity-planning skill compares demand with headcount and load | Restricted to planning roles; read-only HRIS | Headcount plan grounded in demand; fewer mis-hires | Cost control |
| 7 | Ratings history + evidence packs | Calibration pack with distribution checks | Restricted scope; fairness review each cycle | Faster, fairer calibration | Performance, Risk |

### 4.3 KPI map

| Lever | Leading KPI (input, moves first) | Lagging KPI (output, moves the P&L) | Bottom-line translation | Measured from |
|-------|----------------------------------|-------------------------------------|-------------------------|---------------|
| Efficiency | Manager hours per review cycle; hours in calibration | Management time returned to the team | Hours saved × managers × cycles per year × loaded hourly cost | Time survey, analytics |
| Performance | Goals with current status; 1:1s held as scheduled | Goal attainment; team throughput and cycle time | Δ throughput × value per delivered item (customer's own model) | Goal and work-tracking systems |
| Cost control | Teams flagged for overload and supported | Regretted attrition; absence days | Avoided leavers × replacement cost (recruiting + ramp time) + Δ absence days × daily cost | HRIS |
| Cost control | Headcount plan backed by demand data | Hiring against forecast need; contractor spend | Avoided unnecessary hires × fully loaded cost; Δ contractor spend | HRIS, finance |
| Risk | Audited share of people-data access by AI; fairness checks run | Rating appeals and employment disputes | Avoided dispute and legal cost | Audit log, HR case records |

### 4.4 Expected benefit

**Net annual value = sum of the lever values in 4.3 − (platform cost + AI usage cost + delivery cost).** Every input comes from the customer's own baseline, taken in discovery (section 7), and is re-measured at 30, 60 and 90 days. Quote results only from measured data: `[INSERT: measured result from customer deployment]`.

## 5. Platform capabilities used

- **MCP servers** for the HRIS, performance and goal tools, project trackers, code repositories, CRM, help desk and calendar, each with its own scoped tool exposure, OAuth and access log. See [concepts/mcp.md](../../concepts/mcp.md).
- **Identity and authorisation**: OIDC SSO to the customer IdP, so a manager's request carries their identity and the default-deny hook decides which people data they may reach. See [concepts/authentication.md](../../concepts/authentication.md).
- **Skills and plugins** for review prep, 1:1 prep, goal tracking and calibration, scoped to HR and manager roles. See [guides/marketplace-authoring.md](../../guides/marketplace-authoring.md).
- **A2A agents** for recurring people-analytics work. See [concepts/a2a-protocol.md](../../concepts/a2a-protocol.md).
- **Scheduled jobs** for goal-drift and overload digests (`infra jobs`). See [reference/cli.md](../../reference/cli.md).
- **Audit** of every tool call and request, so the customer can show who accessed whose data and why. See [concepts/gateway.md](../../concepts/gateway.md).
- **OTLP export** into the customer's BI or people-analytics stack. See [guides/operate.md](../../guides/operate.md).

## 6. Reference use cases

**Review season.** Before: managers spend days reconstructing a year from memory and scattered tools; reviews skew toward the last month. After: each manager runs a review-prep skill per report and receives an evidence pack covering the whole period, with sources linked. The manager writes the review; the evidence is the starting point, not the verdict.

**Quarterly goal check.** Before: OKR status is updated the week before the business review. After: a scheduled skill reconciles goals with linked work items every week and notifies owners when a goal drifts.

**Team health review.** Before: HR learns a team is overloaded from exit interviews. After: the HR business partner gets a monthly team-level digest of work in progress, out-of-hours activity and meeting load, and raises it with the team lead while there is time to act.

**Headcount planning.** Before: each department asks for more people with a narrative. After: planning starts from a shared view of demand and current load per team, drawn from the same systems for every department.

## 7. Implementation pattern

1. **Discover**: agree the decisions this pillar supports (reviews, goals, planning, wellbeing); inventory the source systems; agree the privacy, fairness and access model with HR, legal, the DPO and employee representatives **before** anything is connected.
2. **Configure**: MCP servers to source systems, read-only, with per-user authorisation so a manager reaches only their own reports' data; SSO claims for reporting lines; skills for review prep, 1:1 prep and goal tracking; team-level aggregation for productivity and overload views.
3. **Pilot**: one function for one review or planning cycle; publish to employees what is collected, what is not, and who can see it.
4. **Roll out**: extend function by function; add calibration and capacity planning once review prep is trusted.
5. **Measure**: manager time per cycle, dispute rates, goal freshness, attrition and absence against baseline; re-run the fairness checks every cycle.

Typical deliverables: people-data access and privacy model (signed off by HR, DPO and employee representatives), source-system and scope register, review and goal skill set, team productivity dashboard, fairness review procedure, cycle-by-cycle value review.

## 8. Risks and governance guardrails

- **People decide, AI prepares.** Skills assemble evidence and drafts. Ratings, pay, promotion and dismissal decisions stay with people, and the skill output says so.
- **Regulation**: AI used to evaluate or monitor workers is regulated in many jurisdictions (the EU AI Act lists it among high-risk uses, and works-council consultation or consent is often required). Establish the customer's obligations with their legal team before rollout. The platform supplies the access control and audit evidence those obligations usually require; it does not discharge them.
- **No keystroke surveillance.** Measure outcomes from work systems (delivered items, cycle time, cases resolved), not activity traces. Do not connect screen, keystroke or mouse-activity sources.
- **Team level by default.** Productivity and overload views aggregate at team level. Individual-level data is limited to the person, their manager and named HR roles, for a documented purpose.
- **Metrics are not performance.** Ticket counts and commits vary by role and by the kind of work. Use them as evidence to discuss, never as a score. Activity counts and AI usage must not be used as a rating on their own.
- **Fairness**: check rating and evidence distributions across teams and groups each cycle; correct skills that systematically under-represent some kinds of work (mentoring, incident response, documentation).
- **Least privilege**: HRIS access is the most sensitive scope on the platform. Grant it to the fewest skills and roles possible, read-only, and review the audit trail of HRIS reads every month.

## 9. Certification objectives (`SP-SPx-PPL`)

Candidates can:
- Facilitate the privacy, fairness and access-model workshop with HR, legal and employee representatives.
- Design MCP access to HR and work systems so each manager reaches only their own reports' data, and prove it from the audit trail.
- Build a review-prep skill that produces a period-wide, source-linked evidence pack.
- Configure team-level productivity and overload views and explain the limits of each metric.
- Explain which decisions must stay with people, and how the configuration enforces that.
- Build the value map for a customer: data sources to connect, implementation, controls, levers, and the value formula filled with the customer's own baseline.

Lab: on a demo tenant with a sandbox HRIS, tracker and code repository, configure per-manager access, generate a review evidence pack for one report, show that a second manager is denied the same data, and produce a team-level throughput and meeting-load view with the audit trail of every read.
