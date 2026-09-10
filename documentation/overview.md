# Overview

What systemprompt-core is, what it does, and when to deploy it.

systemprompt-core is a self-hosted system for running AI agents and MCP servers under a single governed boundary. It compiles to one Rust binary that you run on infrastructure you control, backed by a PostgreSQL database you own. The gateway, MCP and agent interfaces apply their configured authentication, authorization and audit controls. Enforcement applies to traffic routed through those interfaces.

Operators configure identity, secrets, provider endpoints and storage. Enabled providers and integrations can send requests to external services. PostgreSQL stores application records; profiles, secrets and file-backed artifacts also require deployment storage.

## What it does

systemprompt-core provides five capabilities behind one HTTP surface:

- **A2A (agent-to-agent) protocol.** A standalone agent server speaks the A2A JSON-RPC protocol with server-sent-event streaming and `.well-known` discovery. Agents are described as configuration and registered in a central registry.
- **MCP (Model Context Protocol) servers.** MCP servers use streamable HTTP and can run as managed subprocesses behind the API proxy. Each server has its own scoped tool exposure, OAuth2, and access log, discoverable through a central registry.
- **OAuth2 / OIDC authorization server.** A built-in authorization server issues and validates tokens for the system's own surfaces. It supports OIDC discovery, PKCE (S256), and WebAuthn. The JWT plane is RS256.
- **Provider gateway.** A provider-facing proxy exposes a stable `/v1` surface (`POST /v1/messages`, `POST /v1/chat/completions`, `POST /v1/responses`, `GET /v1/models`) and routes each model pattern to a configured upstream provider. The upstream is selected in configuration, not in code.
- **Compile-time extensions.** Functionality is extended in Rust through the `Extension` trait, registered at compile time with the `inventory` crate. There is no runtime plugin loader and no `dlopen`; extension code compiles into your binary.

Across all five, the request path enforces authorization through a fail-closed authorization hook (default deny), rate limiting, and structured audit logging. Decisions are recorded with a `trace_id` so a single agent action can be reconstructed end to end.

## When to use it

Deploy systemprompt-core when you need to:

- Run AI agents or MCP tools against internal systems and retain the audit record on your own infrastructure.
- Manage provider credentials and deployment secrets under your own key-management lifecycle.
- Standardize many AI clients (Claude Code, an Anthropic-SDK application, any MCP host) on one governed endpoint.
- Switch inference providers without changing application code.

It is not a hosted SaaS and not a sidecar. It is a binary plus a database that you operate. If you want a managed service, or you do not need a self-hosted audit boundary, this is more than you need.

## Deployment model

The deployment comprises:

1. **Runtime processes.** The API and managed agent/MCP processes use configured ports and storage paths. Multiple API replicas require shared database access, coordinated jobs and appropriate routing.
2. **PostgreSQL 18+.** Application records, identities, tasks, contexts and audit data.
3. **Configuration and file storage.** Profiles, secrets and file-backed artifacts need explicit persistence and backup policies.

Secrets are customer-owned. The binary performs no symmetric at-rest encryption of the secrets file; it receives plaintext only after your own tooling (KMS, HSM, Vault, or sops) opens the envelope. The master key never enters the binary.

Configuration is a profile — a `profile.yaml` document plus a referenced secrets source. Bootstrap runs in a fixed order: load and interpolate the profile, load secrets, materialize provider credentials, build validated config, then assemble the service graph. There are no environment-variable fallbacks for configuration that the profile is responsible for; the profile is the source of truth.

```
        AI clients                 Provider gateway
  (Claude Code, SDK apps,   ┌────────────────────────► upstream providers
   MCP / A2A hosts)         │   (Anthropic, OpenAI,
        │                   │    Gemini, custom)
        ▼                   │
  ┌──────────────────────────────────┐
  │        systemprompt-core          │   configured runtime
  │  auth → authz hook → rate limit   │   (run N replicas)
  │  → audit → A2A / MCP / OAuth      │
  └──────────────────────────────────┘
        │
        ▼
   PostgreSQL 18+        ◄── the only durable state (you own it)
```

The binary requires no outbound network access for governance operation. In an air-gapped network it talks only to PostgreSQL, plus whatever provider endpoints you configure (which can be an internal proxy).

## How to read the rest of this documentation

- New to the system — start with [getting-started.md](getting-started.md), a single path from a clean machine to a running server.
- Running it in production — see [guides/deploy-production.md](guides/deploy-production.md) for high availability, backup, key rotation, and monitoring.
- Evaluating it for security or procurement — the security and reference material at the top level of this directory covers the threat model, compliance mappings, and stability guarantees.

## Glossary

| Term | Meaning |
|------|---------|
| **profile** | The `profile.yaml` configuration unit plus its referenced secrets source. The single source of truth for how a deployment runs. |
| **extension** | A compile-time `Extension` implementation registered through the `inventory` crate. Distinct from the user-facing "plugin" CLI and marketplace surface. |
| **the gateway** | The provider-facing proxy on the `/v1` base that routes model patterns to upstream inference providers. |
| **authorization hook / authz hook** | The fail-closed (default-deny) check evaluated in the request path before an action is allowed. |
| **audit log / governance decisions** | The append-only record of authorization decisions and system events, written to PostgreSQL and correlated by `trace_id`. |
| **A2A** | Agent-to-agent protocol: JSON-RPC with SSE streaming and `.well-known` discovery. |
| **MCP** | Model Context Protocol: the tool/resource protocol the system hosts over streamable HTTP. |
| **trace_id** | The correlation identifier that links every log line, execution step, and artifact for one request. |
