# Pillar 4: Governance, Security & Compliance

Control over who can use which AI capability, enforcement on the request path, and an audit record on infrastructure the customer owns. This is the pillar the others depend on: a sales or engineering rollout that a security team cannot defend does not survive procurement.

## 1. Who it's for

- **Buyer**: CISO, CIO, Data Protection Officer, General Counsel, Head of Risk.
- **Users**: security engineering, GRC and compliance teams, internal audit, IT administrators.
- **Partner role delivering it**: consultant holding `SP-SPx-GOV`, with a Solution Architect (`SP-ARC`) on any regulated deployment.

## 2. Business problems solved

1. Shadow AI: staff use consumer AI tools and personal keys; company data leaves without a record.
2. No single place to decide which people may use which models, tools and agents.
3. When something goes wrong (a leaked credential, a wrong action by an agent) there is no trace that reconstructs what happened.
4. Security questionnaires and audits ask about AI usage and the answers are improvised.
5. Data residency and sovereignty rules rule out sending everything to a SaaS control plane.
6. Agents with tool access raise a new question: who authorised that action?

## 3. Benefits delivered

| Benefit | Mechanism | Enabling capability | KPI | How to baseline |
|---------|-----------|---------------------|-----|-----------------|
| Identity on every AI request | Requests to the gateway, MCP servers and agents are authenticated against the customer IdP | OAuth2/OIDC server, PKCE, WebAuthn, SSO | Share of AI traffic that is identity-bound | Proxy or CASB logs of AI domains |
| Default-deny access control | An authorisation hook evaluates each action before it runs and denies by default | Fail-closed authz hook, scopes, marketplace scoping by role | Policy coverage of models, tools and agents | Current access model |
| Reduced leakage | Requests are screened for credential patterns and blocked content; outbound endpoints are validated | Gateway safety screening, policy, blocklists, SSRF guard | Detections and blocks per month; incidents | Secret-scanning and DLP history |
| Reconstructable incidents | Decisions, tool calls, requests and usage are correlated by `trace_id` in PostgreSQL | Audit log, `infra logs traces`, `infra logs requests`, `infra logs tools` | Time to reconstruct an AI-related incident | Tabletop exercise before rollout |
| Faster questionnaires and audits | Controls are mapped to HIPAA, SOC 2 and ISO 27001 with evidence locations | [security/compliance-control-matrix.md](../../security/compliance-control-matrix.md), [security/threat-model.md](../../security/threat-model.md) | Questionnaire turnaround; audit findings on AI | Last questionnaire cycle time |
| Data residency by design | Self-hosted binary plus customer PostgreSQL; secrets under the customer's KMS or Vault; air-gap capable | Deployment model, Vault/OpenBao secrets, configurable providers | Data stores outside approved regions | Data-flow map |
| Signed, controlled distribution | Plugins, skills, agents and MCP allowlists reach desktops as a signed manifest | Bridge manifest signature verification | Unapproved plugins on endpoints | Endpoint inventory |
| Rate and abuse limits | Rate limits and quotas per subject | Gateway quota, rate limiting | Abuse events; runaway-agent cost incidents | Incident history |

## 4. Value map

How this pillar turns connected data into movement on the bottom line. Read every row of the value chain as **data source + implementation + control = expected benefit**, then follow the benefit to its lever and KPI. Levers and value formulas are defined in the [value model](../value-model.md); every connector is described in the [connectors catalogue](../connectors.md) and every KPI in the [KPI catalogue](../kpis.md).

### 4.1 Data sources to connect

| Data source | Example systems | Data used | Access | Connected via | Data owner |
|-------------|-----------------|-----------|--------|---------------|------------|
| Identity provider | Okta, Entra ID, Google Workspace, Ping | Users, groups, roles, MFA state | Read (OIDC) | OIDC SSO | IT, identity team |
| Secrets and key management | HashiCorp Vault, OpenBao, AWS/GCP/Azure KMS, sops | Provider keys, signing keys | Read at boot | Secrets source in the profile | Security |
| SIEM and log platform | Splunk, Sentinel, Elastic, Datadog | Receives audit events, logs, traces | Write (export) | Structured logs, OTLP export | Security operations |
| Network and egress controls | Firewall, proxy, CASB | Direct provider traffic to block | Configured alongside | Network policy | Network team |
| Model providers | Anthropic, OpenAI, Gemini, internal proxy | Approved inference endpoints | Routed | Gateway routes and allowlist | Platform team |
| Data classification and policy | DLP rules, data catalogue, acceptable-use policy | Blocked patterns, restricted data classes | Encoded as policy | Gateway policy, blocklists, request guards | GRC, DPO |
| GRC tooling | Vanta, Drata, OneTrust, questionnaires | Control evidence requests | Evidence supplied | Control matrix plus audit exports | GRC |

### 4.2 Value chain

| # | Data source (input) | + Implementation | + Control | = Expected benefit (output) | Lever |
|---|---------------------|------------------|-----------|-----------------------------|-------|
| 1 | Identity provider | SSO on gateway, MCP servers and agents | Default-deny authorisation hook; roles from IdP groups | Every AI request tied to a person | Risk |
| 2 | Network controls + model providers | Approved clients routed through the gateway; direct provider access blocked | Provider allowlist; SSRF guard on outbound routes | Shadow AI consolidated onto one governed path | Risk, Cost control |
| 3 | Data classification + policy | Safety screening, credential-pattern detection, blocklists, custom request guards | Block, log and alert | Fewer leaks of secrets and restricted data | Risk |
| 4 | All governed traffic | Audit records correlated by `trace_id`, exported to the SIEM | Append-only audit; retention set to policy | Any AI action reconstructable in minutes | Risk, Efficiency |
| 5 | Secrets management | Provider and signing keys held in the customer's KMS or Vault | Master key never enters the binary | Keys under the customer's own lifecycle | Risk |
| 6 | GRC tooling + audit exports | Questionnaires answered from the control matrix with live evidence | Reviewed by GRC; no certification over-claims | Faster security reviews; deals unblocked | Efficiency, Revenue |

### 4.3 KPI map

| Lever | Leading KPI (input, moves first) | Lagging KPI (output, moves the P&L) | Bottom-line translation | Measured from |
|-------|----------------------------------|-------------------------------------|-------------------------|---------------|
| Risk | Identity-bound share of AI traffic; policy coverage of models and tools | AI-related security and data-protection incidents | Avoided incidents × customer's incident cost (response, notification, fines) | Audit log, proxy logs |
| Efficiency | Time to reconstruct an AI incident; questionnaire turnaround | Security and audit team hours on AI | Hours saved × loaded hourly cost | Tabletop exercises, GRC records |
| Revenue | Security reviews completed per quarter | Deals delayed by security review | Δ days delayed × pipeline value × cost of delay; deals no longer lost to security | CRM, GRC records |
| Cost control | Unmanaged AI tools retired | Duplicate AI licences and tooling | Retired licence and tool spend | Finance |

### 4.4 Expected benefit

**Net annual value = sum of the lever values in 4.3 − (platform cost + AI usage cost + delivery cost).** Every input comes from the customer's own baseline, taken in discovery (section 7), and is re-measured at 30, 60 and 90 days. Quote results only from measured data: `[INSERT: measured result from customer deployment]`.

## 5. Platform capabilities used

- **Authentication and authorisation**: [concepts/authentication.md](../../concepts/authentication.md).
- **Gateway controls**: [concepts/gateway.md](../../concepts/gateway.md).
- **MCP governance and signed manifests**: [concepts/mcp.md](../../concepts/mcp.md).
- **Security pack**: [security/threat-model.md](../../security/threat-model.md), [security/compliance-control-matrix.md](../../security/compliance-control-matrix.md), [security/outbound-egress-controls.md](../../security/outbound-egress-controls.md), [security/stability-contract.md](../../security/stability-contract.md).
- **Secrets**: [guides/vault-secrets.md](../../guides/vault-secrets.md).
- **Production posture**: [guides/deploy-production.md](../../guides/deploy-production.md).
- **Gateway request guards** for organisation-specific decisions before inference dispatch: [guides/authoring-extensions.md](../../guides/authoring-extensions.md).

## 6. Reference use cases

**Shadow-AI consolidation.** Before: dozens of AI tools and keys, no inventory. After: approved clients route through the gateway with SSO; network policy blocks direct provider access; the remaining exceptions are a short, owned list.

**Agent action accountability.** Before: an agent updated a record and nobody can say who asked it to. After: the audit trail shows the user, the agent, the authorisation decision and the tool call, linked by one `trace_id`.

**Security questionnaire response.** Before: each questionnaire is answered from scratch. After: the GRC team answers from the control matrix and attaches evidence from the deployed configuration.

**Regulated deployment.** Before: AI was blocked outright for a regulated business unit. After: a self-hosted tenant in an approved region, providers restricted to approved endpoints (or an internal proxy), secrets in the customer's KMS.

## 7. Implementation pattern

1. **Discover**: AI usage inventory; data classification; regulatory obligations; current IdP, KMS and logging stack.
2. **Design** (Architect-led): deployment topology, identity model, provider allowlist, egress rules, retention, evidence plan.
3. **Configure**: SSO, roles, policy, safety screening, quotas, MCP scopes, marketplace scoping, log export to the SIEM.
4. **Validate**: tabletop exercise reconstructing a simulated incident from the audit trail; test blocked paths actually block.
5. **Operate**: monthly governance review of denials, detections and exceptions; annual control re-mapping.

Typical deliverables: AI data-flow map, access and policy model, egress and provider allowlist, SIEM integration, evidence pack for auditors, governance operating rhythm.

## 8. Risks and governance guardrails

- **Be exact about scope.** Governance applies to traffic routed through the gateway, MCP and agent interfaces. A model gateway alone cannot govern every action on a laptop. Pair the platform with network controls that stop direct provider access.
- **Heuristic screening** reduces risk; it does not guarantee detection. Keep secret scanning and DLP in place.
- **Do not over-claim certifications.** The control matrix maps controls; it is not a certificate. Direct certification questions to current vendor evidence.
- **Retention vs privacy**: audit retention long enough for investigations, short enough for data-minimisation duties. Document the choice.

## 9. Certification objectives (`SP-SPx-GOV`)

Candidates can:
- Produce an AI data-flow map and identify which flows the platform governs.
- Configure SSO, roles, policy and safety screening, and prove a default-deny outcome.
- Reconstruct an end-to-end agent action from the audit trail using `trace_id`.
- Answer a standard security questionnaire using the control matrix, without over-claiming.
- Design egress and provider allowlists for a regulated tenant.
- Build the value map for a customer: data sources to connect, implementation, controls, levers, and the value formula filled with the customer's own baseline.

Lab: given a demo tenant and a simulated incident (an agent wrote to a record and a credential appeared in a prompt), reconstruct the sequence from the audit trail, identify the missing control, configure it, and demonstrate the action is now denied.
