# systemprompt-core documentation

Technical documentation for [systemprompt-core](https://systemprompt.io), a self-hosted, source-available engine
for running governed AI agents (A2A agents, MCP servers, OAuth2/OIDC, and a provider
gateway) on infrastructure you control.

This set is committed with the repository and versioned alongside it. Every concrete
claim is verified against the source it documents. It is the external counterpart to the
local-only engineering notes kept elsewhere in the repository, which are not published.

## Start here

- **New to systemprompt-core?** Read [overview.md](overview.md), then follow
  [getting-started.md](getting-started.md) to build, configure, and run it.
- **Evaluating it?** [overview.md](overview.md) for scope and fit, then
  [security/](security/) for the threat model, compliance mapping, and stability contract.
- **Operating it?** [guides/deploy-production.md](guides/deploy-production.md) and
  [guides/operate.md](guides/operate.md), with [reference/configuration.md](reference/configuration.md).
- **Building on it?** [concepts/](concepts/) for the model, then
  [guides/authoring-extensions.md](guides/authoring-extensions.md) and [reference/](reference/).

## Layout

The set follows four documentation modes — learning, tasks, reference, and explanation.

### Tutorial & overview
- [overview.md](overview.md) — what it is, capabilities, deployment model, glossary.
- [getting-started.md](getting-started.md) — install → configure → run → first request.

### Concepts (explanation)
- [concepts/architecture.md](concepts/architecture.md) — layered crates, data flow, lifecycle.
- [concepts/authentication.md](concepts/authentication.md) — OAuth2/OIDC, JWT, scopes, authz hook.
- [concepts/a2a-protocol.md](concepts/a2a-protocol.md) — agents, tasks, contexts, streaming.
- [concepts/mcp.md](concepts/mcp.md) — MCP servers, registry, signed manifests.
- [concepts/gateway.md](concepts/gateway.md) — the provider-facing proxy and its controls.
- [concepts/extensions.md](concepts/extensions.md) — the compile-time extension model.

### Guides (how-to)
- [guides/configure.md](guides/configure.md) — write and manage a profile.
- [guides/deploy-production.md](guides/deploy-production.md) — HA, backup, DR, key rotation, air-gap.
- [guides/operate.md](guides/operate.md) — health, metrics, logging, troubleshooting, upgrades.
- [guides/authoring-extensions.md](guides/authoring-extensions.md) — build an extension.
- [guides/configure-providers.md](guides/configure-providers.md) — wire AI providers to the gateway.

### Reference
- [reference/configuration.md](reference/configuration.md) — the full profile schema.
- [reference/http-api.md](reference/http-api.md) — every HTTP endpoint, with auth and errors.
- [reference/cli.md](reference/cli.md) — the `systemprompt` command tree.
- [reference/feature-flags.md](reference/feature-flags.md) — the facade feature matrix.
- [reference/compatibility.md](reference/compatibility.md) — provider, protocol, and runtime versions.

### Security & compliance
- [security/threat-model.md](security/threat-model.md) — STRIDE analysis and residual risk.
- [security/compliance-control-matrix.md](security/compliance-control-matrix.md) — HIPAA / SOC 2 / ISO 27001 mappings, with questionnaire pre-answers.
- [security/stability-contract.md](security/stability-contract.md) — stable vs. tracking surface, versioning, deprecation.
- [security/rfi-readiness-audit.md](security/rfi-readiness-audit.md) — posture snapshot for procurement, with known gaps.

For a vendor evaluation, start with the compliance control matrix; it answers most standard
security-questionnaire questions and cites the supporting code paths. Every claim here can be
verified against the source under `crates/`.

Contact: **ed@systemprompt.io** for licensing, security, and RFI correspondence.

## License

systemprompt-core is distributed under the Business Source License 1.1 (BSL-1.1):
source-available for evaluation, development, and non-production use; production use
requires a commercial licence. Each version converts to Apache-2.0 four years after
publication. See the repository [LICENSE](../LICENSE).
