# KPI catalogue

Every KPI used across the seven pillars, grouped by the bottom-line lever it moves. A **leading KPI** moves first and shows the implementation is working; a **lagging KPI** is the one that reaches the P&L. The bottom-line translation turns the lagging KPI into money using the customer's own figures. Lever definitions and formulas are in the [value model](value-model.md).

## How to measure

| Step | When | What |
|------|------|------|
| Baseline | Discovery, before configuration | Record every leading and lagging KPI in scope from the named source, for at least one full cycle (a quarter where history exists) |
| Agree the source | Discovery | One system of record per KPI; platform analytics only for platform-side KPIs |
| Track leading KPIs | Weekly from pilot start | Confirms adoption and correct configuration before lagging KPIs can move |
| Track lagging KPIs | 30, 60 and 90 days, then quarterly | Compared with baseline; seasonality noted |
| Translate | 90-day value review | Apply the lever formula; subtract platform, AI usage and delivery cost |
| Publish | After customer sign-off | Only measured results: `[INSERT: measured result from customer deployment]` |

## Revenue

More won, expanded or retained revenue, sooner. Formula: Δ conversion or win rate × qualified pipeline value; Δ retention × annual recurring revenue.

| Pillar | Leading KPI (input) | Lagging KPI (output) | Bottom-line translation | Measured from |
|--------|---------------------|----------------------|-------------------------|---------------|
| [REV](pillars/01-revenue-growth.md) | Follow-up sent within 24 hours; stage-to-stage conversion | Win rate; average deal size; sales cycle length | Δ win rate × qualified pipeline value; Δ cycle length × pipeline value × cost of capital | CRM |
| [GOV](pillars/04-governance-security.md) | Security reviews completed per quarter | Deals delayed by security review | Δ days delayed × pipeline value × cost of delay; deals no longer lost to security | CRM, GRC records |
| [CXO](pillars/05-customer-operations.md) | Renewals flagged at risk 90+ days out | Gross and net revenue retention | Δ retention × annual recurring revenue | CRM, finance |

## Performance

Better quality and reliability of the work itself. Formula: Δ output (throughput, attainment, incidents) × value or cost per unit.

| Pillar | Leading KPI (input) | Lagging KPI (output) | Bottom-line translation | Measured from |
|--------|---------------------|----------------------|-------------------------|---------------|
| [REV](pillars/01-revenue-growth.md) | Forecast submitted with evidence; next-step freshness | Forecast accuracy (forecast vs actual) | Fewer missed quarters; less over-hiring or under-hiring against a wrong forecast | CRM, finance |
| [PPL](pillars/02-people-performance.md) | Goals with current status; 1:1s held as scheduled | Goal attainment; team throughput and cycle time | Δ throughput × value per delivered item (customer's own model) | Goal and work-tracking systems |
| [ENG](pillars/03-engineering-productivity.md) | Change failure rate; mean time to restore | Production incidents; SLA breaches | Δ incident hours × cost of downtime per hour | Observability, incident records |
| [CXO](pillars/05-customer-operations.md) | Reopen rate; QA score | CSAT; resolution time | Linked to retention through the customer's CSAT-to-churn data | Help desk, surveys |
| [FIN](pillars/06-ai-finops-operations.md) | Availability; error rate from providers | User-visible AI downtime | Δ downtime hours × users affected × loaded hourly cost | Prometheus, incident records |
| [KNW](pillars/07-knowledge-content.md) | Users on current skill version | Error and rework rate on captured workflows | Δ rework hours × loaded hourly cost | Process quality data |

## Efficiency

The same work in fewer hours, or more work at the same headcount. Formula: Hours saved per person per week × people × working weeks × loaded hourly cost.

| Pillar | Leading KPI (input) | Lagging KPI (output) | Bottom-line translation | Measured from |
|--------|---------------------|----------------------|-------------------------|---------------|
| [REV](pillars/01-revenue-growth.md) | Prep and admin hours per rep per week | Share of time spent selling | Hours saved × reps × working weeks × loaded hourly cost | Time study, `analytics tools` |
| [PPL](pillars/02-people-performance.md) | Manager hours per review cycle; hours in calibration | Management time returned to the team | Hours saved × managers × cycles per year × loaded hourly cost | Time survey, analytics |
| [ENG](pillars/03-engineering-productivity.md) | PR cycle time; review rounds per PR | Lead time for changes; features delivered per sprint | Hours saved × engineers × weeks × loaded hourly cost, or Δ throughput at constant headcount | Repo and tracker data, DORA |
| [GOV](pillars/04-governance-security.md) | Time to reconstruct an AI incident; questionnaire turnaround | Security and audit team hours on AI | Hours saved × loaded hourly cost | Tabletop exercises, GRC records |
| [CXO](pillars/05-customer-operations.md) | First-response time; handling time per case | Cases handled per agent | Hours saved × agents × weeks × loaded hourly cost, or cases absorbed without new hires | Help-desk reports |
| [FIN](pillars/06-ai-finops-operations.md) | Time to add or switch a provider | Engineering effort on provider integration | Hours saved × loaded hourly cost | Change records |
| [KNW](pillars/07-knowledge-content.md) | Skills in production; users per skill | Time per task for captured workflows | Δ time per task × task volume × loaded hourly cost | Time study, `analytics tools` |
| [KNW](pillars/07-knowledge-content.md) | Time for a change to reach all users | Time to adopt a new process or policy | Days saved × affected staff × daily cost of the old process | Marketplace versions, analytics |

## Cost control

Lower or more predictable external spend. Formula: Baseline external spend − post-rollout spend (licences, agencies, overspend, avoided hires, attrition).

| Pillar | Leading KPI (input) | Lagging KPI (output) | Bottom-line translation | Measured from |
|--------|---------------------|----------------------|-------------------------|---------------|
| [REV](pillars/01-revenue-growth.md) | Agency and freelance content spend; AI cost per rep | Cost of sale | Δ agency spend + Δ AI spend against budget | Finance, `analytics costs` |
| [PPL](pillars/02-people-performance.md) | Teams flagged for overload and supported | Regretted attrition; absence days | Avoided leavers × replacement cost (recruiting + ramp time) + Δ absence days × daily cost | HRIS |
| [PPL](pillars/02-people-performance.md) | Headcount plan backed by demand data | Hiring against forecast need; contractor spend | Avoided unnecessary hires × fully loaded cost; Δ contractor spend | HRIS, finance |
| [ENG](pillars/03-engineering-productivity.md) | AI cost per engineer; share of traffic on the gateway | AI spend vs budget | Spend avoided by quotas and model tiering; consolidated licences | `analytics costs`, invoices |
| [GOV](pillars/04-governance-security.md) | Unmanaged AI tools retired | Duplicate AI licences and tooling | Retired licence and tool spend | Finance |
| [CXO](pillars/05-customer-operations.md) | Deflection rate on internal desks | Tickets per employee; service-desk headcount need | Deflected tickets × cost per ticket | Service-desk reports |
| [FIN](pillars/06-ai-finops-operations.md) | Attributed share of AI spend; quota hits | AI spend vs budget; cost per completed workflow | Δ cost per workflow × workflow volume; overspend avoided | `analytics costs`, invoices |
| [FIN](pillars/06-ai-finops-operations.md) | Share of requests on lower-cost tiers | Blended cost per 1,000 requests | Δ blended unit cost × request volume | Gateway audit and pricing |
| [KNW](pillars/07-knowledge-content.md) | Duplicate skills retired; content review rounds | Content and enablement spend | Δ agency and enablement spend | Finance |

## Risk

Lower probability or cost of a damaging event. Formula: Δ incident frequency × average incident cost.

| Pillar | Leading KPI (input) | Lagging KPI (output) | Bottom-line translation | Measured from |
|--------|---------------------|----------------------|-------------------------|---------------|
| [REV](pillars/01-revenue-growth.md) | Share of CRM access by AI that is audited | Data incidents involving customer data | Avoided incident cost (customer's own incident cost model) | Audit log |
| [PPL](pillars/02-people-performance.md) | Audited share of people-data access by AI; fairness checks run | Rating appeals and employment disputes | Avoided dispute and legal cost | Audit log, HR case records |
| [ENG](pillars/03-engineering-productivity.md) | Credential-pattern detections; personal keys retired | Security incidents involving AI tools | Avoided incident cost | Audit log, security records |
| [GOV](pillars/04-governance-security.md) | Identity-bound share of AI traffic; policy coverage of models and tools | AI-related security and data-protection incidents | Avoided incidents × customer's incident cost (response, notification, fines) | Audit log, proxy logs |
| [CXO](pillars/05-customer-operations.md) | Customer-facing AI errors caught in QA | Complaints and credits issued due to wrong answers | Avoided credits and complaint handling | Help desk, finance |
| [FIN](pillars/06-ai-finops-operations.md) | Provider concentration | Exposure to one provider's outage or price change | Business continuity (qualitative, customer's risk register) | Gateway routes, contracts |
| [KNW](pillars/07-knowledge-content.md) | Critical workflows with an owned skill | Knowledge loss when people leave | Avoided ramp-up time for replacements × loaded cost | HRIS, skill inventory |
