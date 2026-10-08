# Pillar 6: AI FinOps & Platform Operations

AI spend that finance can attribute, forecast and cap, and an AI platform that IT can operate like any other production service: health checks, metrics, logs, upgrades and provider choice under configuration rather than code.

## 1. Who it's for

- **Buyer**: CFO, CIO, Head of Platform, Head of IT Operations, FinOps lead.
- **Users**: finance business partners, FinOps analysts, platform engineers, SREs, IT administrators.
- **Partner role delivering it**: Administrator plus consultant holding `SP-SPx-FIN`.

## 2. Business problems solved

1. AI invoices arrive from several providers and card charges with no breakdown by team, project or use case.
2. Budgets are blown by a few heavy users or runaway agents, and nobody notices until the invoice.
3. The business is locked into one provider because switching means changing every application.
4. Models with very different prices are used for the same work, with no policy on which model fits which job.
5. The AI stack is not run like production: no health checks, no metrics, no upgrade discipline.
6. Provider outages and rate limits break user workflows.

## 3. Benefits delivered

| Benefit | Mechanism | Enabling capability | KPI | How to baseline |
|---------|-----------|---------------------|-----|-----------------|
| Cost attribution | Every gateway request records identity, model, token counts and pricing | Gateway audit and pricing, `analytics costs`, per-user and platform cost views | Share of AI spend attributed to a team or cost centre | Current invoices vs any internal allocation |
| Budget enforcement | Subject-keyed quotas reject requests once a window is exhausted | Gateway quota and rate limiting | Spend vs budget per team; overspend events | Last two quarters' variance |
| Right model for the job | Model patterns route to providers by configuration; cheaper models can be scoped to routine work | Gateway routes, provider registry, model catalogue (`/v1/models`) | Cost per request or per completed workflow by model | Current model mix and unit cost |
| Provider independence | Applications speak one `/v1` surface; upstream is chosen in configuration | Inbound adapters (Anthropic, OpenAI Chat, OpenAI Responses), outbound adapters incl. Gemini | Effort to add or switch a provider; provider concentration | Engineering effort of last provider change |
| Resilience to provider limits | Bounded retry with backoff on transient 429 and 503; multiple providers configured | Gateway retry policy | User-visible failures from provider errors | Error rate from current tools |
| Production-grade operation | Liveness and readiness probes, Prometheus metrics, structured logs, OTLP export, graceful draining, documented upgrade and rollback | [guides/operate.md](../../guides/operate.md), [guides/deploy-production.md](../../guides/deploy-production.md) | Availability; mean time to detect and recover | Current incident history |
| Forecastable spend | Usage trends by team and model feed the forecast | Analytics export, OTLP to BI | Forecast error on AI spend | Last forecast vs actual |

## 4. Value map

How this pillar turns connected data into movement on the bottom line. Read every row of the value chain as **data source + implementation + control = expected benefit**, then follow the benefit to its lever and KPI. Levers and value formulas are defined in the [value model](../value-model.md); every connector is described in the [connectors catalogue](../connectors.md) and every KPI in the [KPI catalogue](../kpis.md).

### 4.1 Data sources to connect

| Data source | Example systems | Data used | Access | Connected via | Data owner |
|-------------|-----------------|-----------|--------|---------------|------------|
| Model provider accounts | Anthropic, OpenAI, Gemini, Azure OpenAI, internal | Inference, prices | Routed | Gateway routes and provider registry | Platform team |
| Provider invoices and contracts | Provider billing, procurement system | Committed spend, unit prices | Read for reconciliation | Finance process | Finance, procurement |
| Identity provider | Okta, Entra ID | Team and cost-centre claims | Read (OIDC) | OIDC SSO | IT |
| Finance and BI | ERP, cost-centre structure, BI tool | Budgets, chargeback targets | Receives exports | OTLP and analytics exports | Finance |
| Monitoring stack | Prometheus, Grafana, Datadog, PagerDuty | Health, metrics, alerts | Scrape and export | Metrics endpoint, structured logs, OTLP | SRE |
| Infrastructure | Kubernetes, VMs, managed PostgreSQL, backup | Runtime, database, backups | Operated | Deployment configuration | Platform team |

### 4.2 Value chain

| # | Data source (input) | + Implementation | + Control | = Expected benefit (output) | Lever |
|---|---------------------|------------------|-----------|-----------------------------|-------|
| 1 | Provider accounts + IdP claims | Every request metered with identity, model, tokens and price | Requests without identity are refused | Spend attributed to team and cost centre | Cost control |
| 2 | Budgets from finance | Quotas per team and per agent | Enforced at the gateway; alerts before the ceiling | No surprise invoices; runaway agents stopped | Cost control |
| 3 | Model catalogue + prices | Model tiering policy routes routine work to lower-cost models | Routes in configuration; quality checked against the pillar's outcome KPIs | Lower cost per completed task | Cost control, Efficiency |
| 4 | Several provider accounts | Second provider configured; routes moved by configuration | Provider allowlist | No single-provider outage stops work; stronger negotiating position | Risk, Cost control |
| 5 | Monitoring stack | Probes, metrics, logs, OTLP; upgrade and rollback runbook | Alerting thresholds; change control | AI platform run like production | Performance, Risk |
| 6 | Invoices + gateway records | Monthly reconciliation and chargeback | Finance sign-off | Spend forecastable; budgets owned by teams | Cost control |

### 4.3 KPI map

| Lever | Leading KPI (input, moves first) | Lagging KPI (output, moves the P&L) | Bottom-line translation | Measured from |
|-------|----------------------------------|-------------------------------------|-------------------------|---------------|
| Cost control | Attributed share of AI spend; quota hits | AI spend vs budget; cost per completed workflow | Δ cost per workflow × workflow volume; overspend avoided | `analytics costs`, invoices |
| Cost control | Share of requests on lower-cost tiers | Blended cost per 1,000 requests | Δ blended unit cost × request volume | Gateway audit and pricing |
| Efficiency | Time to add or switch a provider | Engineering effort on provider integration | Hours saved × loaded hourly cost | Change records |
| Performance | Availability; error rate from providers | User-visible AI downtime | Δ downtime hours × users affected × loaded hourly cost | Prometheus, incident records |
| Risk | Provider concentration | Exposure to one provider's outage or price change | Business continuity (qualitative, customer's risk register) | Gateway routes, contracts |

### 4.4 Expected benefit

**Net annual value = sum of the lever values in 4.3 − (platform cost + AI usage cost + delivery cost).** Every input comes from the customer's own baseline, taken in discovery (section 7), and is re-measured at 30, 60 and 90 days. Quote results only from measured data: `[INSERT: measured result from customer deployment]`.

## 5. Platform capabilities used

- **Gateway**: routing, catalogue, quota, pricing, audit, retry. See [concepts/gateway.md](../../concepts/gateway.md) and [guides/configure-providers.md](../../guides/configure-providers.md).
- **Analytics**: `analytics costs`, `analytics requests`, `analytics overview`. See [reference/cli.md](../../reference/cli.md).
- **Operations**: health probes, metrics, logs, OTLP, upgrades. See [guides/operate.md](../../guides/operate.md).
- **Deployment**: HA, backup, DR, key rotation. See [guides/deploy-production.md](../../guides/deploy-production.md).
- **Compatibility**: supported providers and protocol versions. See [reference/compatibility.md](../../reference/compatibility.md).

## 6. Reference use cases

**Chargeback.** Before: AI cost sits in one IT cost centre and is argued about annually. After: monthly cost by team and cost centre from gateway records, reconciled to provider invoices.

**Runaway agent.** Before: a looping agent burns a month's budget over a weekend. After: the agent's subject has a quota; requests are rejected at the ceiling and an alert fires from metrics before it is reached.

**Model tiering.** Before: the most expensive model is used for everything. After: routine summarisation routes to a lower-cost model, complex reasoning to a frontier model; cost per workflow is compared in analytics.

**Second provider.** Before: a provider outage stops work. After: a second provider is configured; routes can be moved in configuration.

## 7. Implementation pattern

1. **Discover**: collect AI invoices and accounts; agree the attribution dimensions (team, cost centre, project); baseline spend and variance.
2. **Configure**: providers and routes; model tiering policy; quotas per team; SSO claims that carry team and cost centre; metrics and log export.
3. **Pilot**: shadow-mode reporting for a month (report, do not enforce); reconcile against invoices.
4. **Roll out**: enforce quotas; publish monthly chargeback; move remaining traffic onto the gateway.
5. **Operate**: monthly FinOps review; quarterly model and provider review; upgrade cadence aligned to releases.

Typical deliverables: provider and route configuration, model tiering policy, quota table, chargeback model, operations runbook, monitoring dashboards.

## 8. Risks and governance guardrails

- **Quota overshoot**: cost is accounted after completion, so concurrent requests can exceed a ceiling. Leave headroom and alert early.
- **Pricing drift**: provider prices change. Review pricing configuration on each provider price change and reconcile to invoices monthly.
- **Silent downgrade**: routing work to a cheaper model can lower quality. Pair cost KPIs with the outcome KPIs of the pillar the work belongs to.
- **Untracked traffic**: spend outside the gateway is invisible. Track the share of AI spend that flows through it.

## 9. Certification objectives (`SP-SPx-FIN`)

Candidates can:
- Configure multiple providers and route model patterns to them.
- Design a model tiering policy and explain its quality risk.
- Configure quotas and explain their overshoot semantics.
- Produce a cost-by-team readout and reconcile it to a provider invoice.
- Set up health probes, metrics scraping and log export for a production tenant.
- Build the value map for a customer: data sources to connect, implementation, controls, levers, and the value formula filled with the customer's own baseline.

Lab: on a demo tenant, add a second provider, move one model pattern to it, set a team quota, exhaust it, then produce a cost readout by team and model and show the corresponding Prometheus metrics.
