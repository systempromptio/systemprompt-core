<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo-dark.svg" alt="systemprompt.io" width="400">
</picture>

### The touchpoint between your AI and everything it does

[![Crates.io](https://img.shields.io/crates/v/systemprompt.svg)](https://crates.io/crates/systemprompt)
[![Docs.rs](https://docs.rs/systemprompt/badge.svg)](https://docs.rs/systemprompt)
[![License: BSL-1.1](https://img.shields.io/badge/License-BSL--1.1-blue.svg)](LICENSE)
[![Rust](https://img.shields.io/badge/rust-1.75+-orange.svg)](https://www.rust-lang.org/)
[![Discord](https://img.shields.io/badge/Discord-Join%20us-5865F2.svg)](https://discord.gg/wkAbSuPWpr)

[Website](https://systemprompt.io) · [About](https://systemprompt.io/about) · [Documentation](https://systemprompt.io/documentation/) · [Live Demo](https://systemprompt.io/features/demo) · [Discord](https://discord.gg/wkAbSuPWpr)

</div>

---

systemprompt.io is a single compiled Rust binary that sits between your AI agents and everything they touch. Every tool call authenticated, authorised, rate-limited, logged, and costed. Self-hosted. Air-gap capable. Provider-agnostic.

One language. One database (PostgreSQL). One binary (~50MB). No microservices. No Kubernetes required. No Redis. No Kafka. No ElasticSearch.

## Table of Contents

- [Why systemprompt.io](#why-systempromptio)
- [Performance](#performance)
- [Quick Start](#quick-start)
- [Core Capabilities](#core-capabilities)
- [Architecture](#architecture)
- [Extensions](#extensions)
- [License](#license)

## Why systemprompt.io

**Govern every tool call.** AI agents take actions on behalf of your people. Without governance, any agent can use any tool, access any data, and leak any credential. systemprompt.io enforces who can do what before it happens, not after.

**Prove every decision.** When the auditor asks what AI did and who authorised it, you query the answer. Full lineage from AI request to tool call to MCP execution, all linked by trace_id. Structured JSON for your SIEM.

**Standardise every team.** Your best AI workflows should not live in one developer's head. systemprompt.io is the skill library for your organisation: curated knowledge, governed plugins, consistent standards.

### What this replaces

| Problem | Without systemprompt.io | With systemprompt.io |
|---------|------------------------|---------------------|
| AI governance | Build from components (months) | Deploy one binary (days) |
| Audit trails | Policy documents | Structured, queryable evidence |
| Secret management | Secrets in context windows | Server-side injection via MCP |
| Cost attribution | No visibility | Per-agent, per-model, per-department |
| Multi-provider | Separate governance per provider | One governance layer for all |

## Performance

200 concurrent governance requests benchmarked. Each performs JWT validation, scope resolution, three rule evaluations, and an async database write.

- **Sub-5ms p50 latency**
- **Sub-10ms p99 latency**
- **Zero garbage collector pauses**
- Throughput supports hundreds of concurrent developers on a single instance

See the [live load test](https://systemprompt.io/features/demo) for full results.

## Quick Start

```bash
# 1. Create from template
gh repo create my-project --template systempromptio/systemprompt-template --clone
cd my-project

# 2. Build
just build

# 3. Login
just login

# 4. Create tenant
just tenant

# 5. Start
just start
```

See [systemprompt-template](https://github.com/systempromptio/systemprompt-template) for full installation instructions.

## Core Capabilities

### Governance Pipeline

Synchronous four-layer evaluation on every tool call. Scope check, secret scan, blocklist, rate limit. All four layers evaluate in the request path. The tool call either passes all four layers and executes, or it is blocked. Single-digit milliseconds overhead.

- [Governance Pipeline](https://systemprompt.io/features/governance-pipeline)
- [Compliance](https://systemprompt.io/features/compliance) (SOC 2, ISO 27001, HIPAA, OWASP Agentic Top 10)

### Secrets Management

Secrets flow through MCP services, not inference endpoints. The agent calls the tool, the MCP service injects the credential server-side. The LLM never sees it. ChaCha20-Poly1305 encryption with per-user key hierarchy.

- [Secrets Management](https://systemprompt.io/features/secrets-management)

### Analytics and Observability

Full audit trail from AI request to tool call to MCP execution to cost. Structured JSON events for Splunk, ELK, Datadog, and Sumo Logic. Cost tracking in microdollars by model, agent, and department.

- [Analytics & Observability](https://systemprompt.io/features/analytics-and-observability)

### MCP Governance

Per-server OAuth2, governed tool calls, central MCP server registry with health monitoring. Built on MCP natively, not proxied. Claude Desktop compatible.

- [MCP Governance](https://systemprompt.io/features/mcp-governance)

### Skill Marketplace

Curated library of your organisation's AI knowledge. Browse, install, create, and fork skills. Plugin bundles with governed distribution by role and department.

- [Skill Marketplace](https://systemprompt.io/features/skill-marketplace)

### Self-Hosted Deployment

Single 50MB binary. Air-gapped, PostgreSQL only. Copy to a server, start it. That is the deployment.

- [Self-Hosted & Air-Gapped](https://systemprompt.io/features/self-hosted-ai-platform)

### Open Standards

- **MCP** (Model Context Protocol) from Anthropic, implemented natively
- **A2A** (Agent-to-Agent Protocol) from Google
- **OAuth2/OIDC** with PKCE, token introspection, audience/issuer checks
- **WebAuthn** for passwordless authentication

### Config as Code

```
services/
├── agents/           # Agent definitions with OAuth scopes
├── mcp/              # MCP servers with per-tool permissions
├── skills/           # Skills and plugins
├── ai/               # Provider configs (Anthropic, OpenAI, Gemini)
├── content/          # Markdown content sources
├── scheduler/        # Cron jobs and background tasks
└── web/              # Theme, branding, navigation
```

### MCP Client Support

Works with any MCP-compatible client: Claude Code, Claude Desktop, ChatGPT, Cursor, and more.

```json
{
  "mcpServers": {
    "my-server": {
      "url": "https://my-tenant.systemprompt.io/api/v1/mcp/my-server/mcp",
      "transport": "streamable-http"
    }
  }
}
```

### Discovery API

| Endpoint | Description |
|----------|-------------|
| `/.well-known/agent-card.json` | Default agent card |
| `/.well-known/agent-cards` | List all available agents |
| `/.well-known/agent-cards/{name}` | Specific agent card |
| `/api/v1/agents/registry` | Full agent registry with status |
| `/api/v1/mcp/registry` | All MCP servers with endpoints |

### CLI

```bash
# Send a message to an agent
systemprompt admin agents message blog "Write a post about MCP security"

# List available MCP tools
systemprompt admin agents tools content-manager

# Deploy to production
systemprompt cloud deploy --profile production
```

## Architecture

Layered crate architecture. Dependencies flow downward only.

```
┌─────────────────────────────────────────────────────────┐
│  ENTRY: api, cli                                        │
├─────────────────────────────────────────────────────────┤
│  APP: runtime, scheduler, generator, sync               │
├─────────────────────────────────────────────────────────┤
│  DOMAIN: users, oauth, ai, agent, mcp, files, content   │
├─────────────────────────────────────────────────────────┤
│  INFRA: database, events, security, config, logging     │
├─────────────────────────────────────────────────────────┤
│  SHARED: models, traits, identifiers, extension         │
└─────────────────────────────────────────────────────────┘
```

Domain crates communicate via traits and events, not direct dependencies.

## Extensions

Build your own extensions by adding the library to your `Cargo.toml`:

```toml
[dependencies]
systemprompt = { version = "0.0.1", features = ["full"] }
```

Available extension traits:

| Trait | Purpose |
|-------|---------|
| `Extension` | Base trait: ID, name, version, dependencies |
| `SchemaExtension` | Database table definitions |
| `ApiExtension` | HTTP route handlers |
| `ConfigExtensionTyped` | Config validation at startup |
| `JobExtension` | Background job definitions |
| `ProviderExtension` | Custom LLM/tool provider implementations |

```rust
use systemprompt_extension::*;

struct MyExtension;
impl Extension for MyExtension { ... }
impl ApiExtension for MyExtension { ... }

register_extension!(MyExtension);
register_api_extension!(MyExtension);
```

Extensions are discovered at compile time via the `inventory` crate. Your code compiles into your binary.

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Converts to Apache 2.0 four years after each version is published.

See [LICENSE](LICENSE) for full terms.

## Links

- [Website](https://systemprompt.io)
- [About](https://systemprompt.io/about)
- [Documentation](https://systemprompt.io/documentation/)
- [Live Demo](https://systemprompt.io/features/demo)
- [Template](https://github.com/systempromptio/systemprompt-template)
- [Discord](https://discord.gg/wkAbSuPWpr)

For licensing enquiries: [ed@systemprompt.io](mailto:ed@systemprompt.io)
