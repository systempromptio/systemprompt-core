# Track: Solution Architect

The technical track that runs across all seven pillars. A Solution Architect designs the platform for an organisation, decides where custom code is justified, and defends the design to a security review. Credential: `SP-ARC` (see [certification-schema.md](../certification-schema.md)).

## What an architect owns in an engagement

- Deployment topology, high availability, backup and disaster recovery.
- Identity design: IdP integration, roles, claims that carry team and cost centre.
- Provider and route design, egress and provider allowlists.
- MCP server design: which systems, which tools, which scopes, in-process or separate process.
- Agent design: A2A agents, their tools and their boundaries.
- Extension design: when to write a compiled extension or a gateway request guard, and when configuration is enough.
- The audit and evidence story, and an honest statement of what the platform does not govern.

## Syllabus

| Module | Content | Sources |
|--------|---------|---------|
| 1. Architecture | Layered crates, request lifecycle, PostgreSQL as the only durable state, profiles and bootstrap order | [concepts/architecture.md](../../concepts/architecture.md), [overview.md](../../overview.md) |
| 2. Identity | OAuth2/OIDC, PKCE, WebAuthn, JWT plane, scopes, the fail-closed authz hook | [concepts/authentication.md](../../concepts/authentication.md) |
| 3. Gateway | Inbound and outbound adapters, canonical request, routing, quota, policy, safety, audit, resilience boundary | [concepts/gateway.md](../../concepts/gateway.md), [guides/configure-providers.md](../../guides/configure-providers.md) |
| 4. MCP | Server lifecycle, registry, streamable HTTP, signed bridge manifests | [concepts/mcp.md](../../concepts/mcp.md) |
| 5. Agents | A2A object model, tasks, contexts, artifacts, streaming, discovery | [concepts/a2a-protocol.md](../../concepts/a2a-protocol.md) |
| 6. Extensions | The `Extension` trait, compile-time registration, schemas and migrations, routes, jobs, providers, gateway request guards | [concepts/extensions.md](../../concepts/extensions.md), [guides/authoring-extensions.md](../../guides/authoring-extensions.md) |
| 7. Distribution | Marketplaces, services bundles, signing | [guides/marketplace-authoring.md](../../guides/marketplace-authoring.md), [guides/services-bundles.md](../../guides/services-bundles.md) |
| 8. Production | HA, backup, DR, key rotation, air-gap, operations, upgrades | [guides/deploy-production.md](../../guides/deploy-production.md), [guides/operate.md](../../guides/operate.md), [guides/vault-secrets.md](../../guides/vault-secrets.md) |
| 9. Security | Threat model, egress controls, compliance mapping, stability contract | [security/](../../security/) |
| 10. Reference | Configuration schema, HTTP API, CLI, feature flags, compatibility | [reference/](../../reference/) |

## Design principles examined at board review

1. **Configuration before code.** Most pillar outcomes need profiles, marketplaces, routes and MCP servers, not extensions. Justify every extension.
2. **Least privilege for tools.** Read-only first. Every write tool has an owner and a reason.
3. **Fail closed.** A policy that cannot be evaluated denies. A partial inventory never resolves to a more permissive default.
4. **Know the boundary.** Governance applies to traffic routed through the gateway, MCP and agent interfaces. State what lies outside it and which compensating control covers it.
5. **Customer-owned state.** Database, secrets and audit records stay in the customer's infrastructure under the customer's keys.
6. **Operable from day one.** Probes, metrics, logs, backup and an upgrade path are part of the design, not a later phase.

## Board review rubric

| Area | Weight |
|------|--------|
| Topology, HA and DR | 15% |
| Identity and authorisation | 20% |
| Gateway, providers and egress | 15% |
| MCP and agent design | 15% |
| Extension decisions | 10% |
| Audit, evidence and compliance mapping | 15% |
| Clarity about what is not governed | 10% |

A candidate who cannot state the governance boundary does not pass, whatever their other scores.
