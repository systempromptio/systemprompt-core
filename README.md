<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo-dark.svg" alt="systemprompt.io" width="400">
</picture>

# Own how your organization uses AI.

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt.io terminal demo" src="assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

### systemprompt-core is the Rust library behind [systemprompt.io](https://systemprompt.io) — the narrow waist between your AI and everything it touches.

[![Crates.io](https://img.shields.io/crates/v/systemprompt.svg?style=flat-square)](https://crates.io/crates/systemprompt)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt?style=flat-square)](https://docs.rs/systemprompt)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](LICENSE)
[![Rust 1.75+](https://img.shields.io/badge/rust-1.75+-f97316?style=flat-square&logo=rust&logoColor=white)](https://www.rust-lang.org/)
[![PostgreSQL 18+](https://img.shields.io/badge/postgres-18+-336791?style=flat-square&logo=postgresql&logoColor=white)](https://www.postgresql.org/)
[![Template](https://img.shields.io/badge/evaluate-systemprompt--template-16a34a?style=flat-square)](https://github.com/systempromptio/systemprompt-template)
[![Discord](https://img.shields.io/badge/Discord-join-5865F2.svg?style=flat-square)](https://discord.gg/wkAbSuPWpr)

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Live Demo**](https://systemprompt.io/features/demo) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

**AI governance infrastructure for agentic systems.** Every tool call authenticated, authorised, rate-limited, logged, and costed. Self-hosted. Air-gap capable. Provider-agnostic. One language. One database (PostgreSQL). One binary (~50 MB). No microservices. No Kubernetes. No Redis. No Kafka.

**AI infrastructure built for AI agents.** A production-ready Rust library with auth, MCP servers, A2A orchestration, and playbooks for deterministic execution. 30 crates published on crates.io under `systemprompt-*` and the `systemprompt` facade. Compile-time extensions via the `inventory` crate. Zero-raw-String-ID policy. Compile-time verified SQL via `sqlx::query!` macros. Typed identifiers across every boundary.

> **Evaluating?** Clone [`systemprompt-template`](https://github.com/systempromptio/systemprompt-template) and run `just build && just setup-local <key> && just start`. 40+ scripted demos exercise every claim below against your own machine in three commands.
>
> **Building on it?** Add `systemprompt = "0.2.1"` to your `Cargo.toml` and jump to [Extensions](#extensions-technical).

## Table of Contents

- [Infrastructure](#infrastructure)
- [Capabilities](#capabilities)
- [Integrations](#integrations)
- [Architecture](#architecture)
- [Extensions (technical)](#extensions-technical)
- [Typed identifiers](#typed-identifiers)
- [Database & repositories](#database--repositories)
- [Facade crate & feature flags](#facade-crate--feature-flags)
- [Performance](#performance)
- [Quick Start](#quick-start)
- [License](#license)

---

## Infrastructure

**One binary. One database. Deploys anywhere.** The same surface local and remote. Config-as-code: agents, MCP servers, skills, AI providers, content, scheduler jobs, and web theme all live as YAML or Markdown under `services/`. Built on open standards: **MCP** (Model Context Protocol), **A2A** (Agent-to-Agent), **OAuth2/OIDC** with PKCE, **WebAuthn**.

**Where in the code**

| Concern | File |
|---|---|
| Bootstrap sequence | `ProfileBootstrap → SecretsBootstrap → CredentialsBootstrap → Config → AppContext` |
| AppContext wiring | [`crates/app/runtime/src/context.rs`](crates/app/runtime/src/context.rs) · [`builder.rs`](crates/app/runtime/src/builder.rs) |
| Provider traits (`LlmProvider`, `ToolProvider`, …) | [`crates/shared/provider-contracts/src/lib.rs`](crates/shared/provider-contracts/src/lib.rs) |
| CLI entry point (8 domains) | [`crates/entry/cli/src/commands/`](crates/entry/cli/src/commands/) |

One binary, eight domains. Every command is discoverable — `systemprompt <domain> --help` works everywhere.

| Domain | Source | Purpose |
|---|---|---|
| `core` | [`crates/entry/cli/src/commands/core/`](crates/entry/cli/src/commands/core/) | Skills, content, files, contexts, plugins, hooks, artifacts |
| `infra` | [`crates/entry/cli/src/commands/infrastructure/`](crates/entry/cli/src/commands/infrastructure/) | Services, database, jobs, logs |
| `admin` | [`crates/entry/cli/src/commands/admin/`](crates/entry/cli/src/commands/admin/) | Users, agents, config, setup, session, rate limits |
| `cloud` | [`crates/entry/cli/src/commands/cloud/`](crates/entry/cli/src/commands/cloud/) | Auth, deploy, sync, secrets, tenant, domain |
| `analytics` | [`crates/entry/cli/src/commands/analytics/`](crates/entry/cli/src/commands/analytics/) | Overview, conversations, agents, tools, requests, sessions, content, traffic, costs |
| `web` | [`crates/entry/cli/src/commands/web/`](crates/entry/cli/src/commands/web/) | Content types, templates, assets, sitemap, validate |
| `plugins` | [`crates/entry/cli/src/commands/plugins/`](crates/entry/cli/src/commands/plugins/) | Extensions, MCP servers, capabilities |
| `build` | [`crates/entry/cli/src/commands/build/`](crates/entry/cli/src/commands/build/) | Build core workspace and MCP extensions |

- [Self-Hosted Deployment](https://systemprompt.io/features/self-hosted-ai-platform)
- [No Vendor Lock-In](https://systemprompt.io/features/no-vendor-lock-in)

## Capabilities

**Every tool call governed.** Synchronous evaluation before execution, not after. Four layers of enforcement in the request path: **scope check → secret detection → blocklist → rate limit**. Deny reasons are structured and auditable. Single-digit milliseconds overhead. No sidecar. No proxy. Compliance that survives an audit: **SOC 2 Type II**, **ISO 27001**, **HIPAA**, **OWASP Top 10 for Agentic Applications**.

**Secrets never touch inference** — the agent calls the tool, the MCP service injects the credential server-side, the LLM never sees it. Per-user key hierarchy encrypted with **ChaCha20-Poly1305**. Every tool call produces a **five-point audit trace**: *Identity → Agent Context → Permissions → Tool Execution → Result*. Everything linked by `trace_id`. Structured JSON events for Splunk, ELK, Datadog, Sumo Logic. Cost tracking in microdollars by model, agent, and department.

**Where in the code**

| Concern | File |
|---|---|
| Scope / RBAC middleware | [`crates/domain/mcp/src/middleware/rbac.rs`](crates/domain/mcp/src/middleware/rbac.rs) |
| Secret detection / scanner | [`crates/infra/security/src/services/scanner.rs`](crates/infra/security/src/services/scanner.rs) |
| Blocklist rules | [`crates/infra/security/src/services/`](crates/infra/security/src/services/) |
| Rate limit middleware (`tower_governor`) | [`crates/infra/security/src/`](crates/infra/security/src/) |
| Audit queries | [`crates/infra/logging/src/trace/audit_queries.rs`](crates/infra/logging/src/trace/audit_queries.rs) |
| Event broadcasters | [`crates/infra/events/src/services/broadcaster.rs`](crates/infra/events/src/services/broadcaster.rs) |
| Secret storage (ChaCha20-Poly1305) | [`crates/infra/security/src/`](crates/infra/security/src/) |
| Typed IDs (`TraceId`, `ContextId`, `TaskId` …) | [`crates/shared/identifiers/src/lib.rs`](crates/shared/identifiers/src/lib.rs) |

**MCP** ([`crates/domain/mcp`](crates/domain/mcp)) is implemented natively — not proxied. Per-server OAuth2, scoped tool exposure, central registry with health monitoring, end-to-end access logs. Works with Claude Code, Claude Desktop, ChatGPT, Cursor, and any other MCP-compatible client.

| Concern | File |
|---|---|
| Orchestrator | [`crates/domain/mcp/src/services/orchestrator/mod.rs`](crates/domain/mcp/src/services/orchestrator/mod.rs) |
| Network / port management / proxy | [`crates/domain/mcp/src/services/network/mod.rs`](crates/domain/mcp/src/services/network/mod.rs) |
| RBAC middleware | [`crates/domain/mcp/src/middleware/rbac.rs`](crates/domain/mcp/src/middleware/rbac.rs) |

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

**Agent-to-Agent** ([`crates/domain/agent`](crates/domain/agent)) ships a standalone A2A server with streaming, a JSON-RPC protocol model, and `.well-known` discovery endpoints.

| Concern | File |
|---|---|
| Standalone A2A server | [`crates/domain/agent/src/services/a2a_server/mod.rs`](crates/domain/agent/src/services/a2a_server/mod.rs) |
| Streaming | [`crates/domain/agent/src/services/a2a_server/streaming/mod.rs`](crates/domain/agent/src/services/a2a_server/streaming/mod.rs) |
| Protocol models (`Message`, `Task`, `TaskState`) | [`crates/domain/agent/src/models/a2a/protocol/mod.rs`](crates/domain/agent/src/models/a2a/protocol/mod.rs) |

**Discovery API**

| Endpoint | Description |
|---|---|
| `/.well-known/agent-card.json` | Default agent card |
| `/.well-known/agent-cards` | List all available agents |
| `/.well-known/agent-cards/{name}` | Specific agent card |
| `/api/v1/agents/registry` | Full agent registry with status |
| `/api/v1/mcp/registry` | All MCP servers with endpoints |

- [Governance Pipeline](https://systemprompt.io/features/governance-pipeline)
- [Secrets Management](https://systemprompt.io/features/secrets-management)
- [MCP Governance](https://systemprompt.io/features/mcp-governance)
- [Analytics & Observability](https://systemprompt.io/features/analytics-and-observability)
- [Closed-Loop Agents](https://systemprompt.io/features/closed-loop-agents)
- [Compliance](https://systemprompt.io/features/compliance)

## Integrations

**Provider-agnostic. Protocol-native. Fully extensible.** Provider-agnostic by trait, not by adapter — swap **Anthropic / OpenAI / Gemini** at the profile level.

- [Any AI Agent](https://systemprompt.io/features/any-ai-agent)
- [Extensible Architecture](https://systemprompt.io/features/extensible-architecture)
- [Skill Marketplace](https://systemprompt.io/features/skill-marketplace)

---

## Architecture

A 30-crate Rust workspace that compiles into a single ~50 MB binary. Dependencies flow downward only — no circular references.

```
┌─────────────────────────────────────────────────────────────────────┐
│  ENTRY      api · cli                                               │
├─────────────────────────────────────────────────────────────────────┤
│  APP        runtime · scheduler · generator · sync                  │
├─────────────────────────────────────────────────────────────────────┤
│  DOMAIN     agent · ai · analytics · content · files · mcp ·        │
│             oauth · templates · users                               │
├─────────────────────────────────────────────────────────────────────┤
│  INFRA      cloud · config · database · events · loader ·           │
│             logging · security                                      │
├─────────────────────────────────────────────────────────────────────┤
│  SHARED     identifiers · provider-contracts · traits ·             │
│             extension · models · client · template-provider        │
└─────────────────────────────────────────────────────────────────────┘
```

All 30 crates publish on crates.io at matching workspace versions. Domain crates communicate via traits and the event bus, not direct dependencies. Database-touching crates ship a per-crate `.sqlx/` query cache (committed) so downstream consumers compile offline — see [`instructions/information/crates-publishing.md`](instructions/information/crates-publishing.md) for the two-level cache convention.

| Layer | Crates |
|---|---|
| Shared | [`systemprompt-identifiers`](https://docs.rs/systemprompt-identifiers) · [`systemprompt-provider-contracts`](https://docs.rs/systemprompt-provider-contracts) · [`systemprompt-traits`](https://docs.rs/systemprompt-traits) · [`systemprompt-extension`](https://docs.rs/systemprompt-extension) · [`systemprompt-models`](https://docs.rs/systemprompt-models) · [`systemprompt-client`](https://docs.rs/systemprompt-client) · [`systemprompt-template-provider`](https://docs.rs/systemprompt-template-provider) |
| Infra | [`systemprompt-database`](https://docs.rs/systemprompt-database) · [`systemprompt-logging`](https://docs.rs/systemprompt-logging) · [`systemprompt-events`](https://docs.rs/systemprompt-events) · [`systemprompt-security`](https://docs.rs/systemprompt-security) · [`systemprompt-loader`](https://docs.rs/systemprompt-loader) · [`systemprompt-config`](https://docs.rs/systemprompt-config) · [`systemprompt-cloud`](https://docs.rs/systemprompt-cloud) |
| Domain | [`systemprompt-analytics`](https://docs.rs/systemprompt-analytics) · [`systemprompt-users`](https://docs.rs/systemprompt-users) · [`systemprompt-files`](https://docs.rs/systemprompt-files) · [`systemprompt-templates`](https://docs.rs/systemprompt-templates) · [`systemprompt-content`](https://docs.rs/systemprompt-content) · [`systemprompt-ai`](https://docs.rs/systemprompt-ai) · [`systemprompt-oauth`](https://docs.rs/systemprompt-oauth) · [`systemprompt-mcp`](https://docs.rs/systemprompt-mcp) · [`systemprompt-agent`](https://docs.rs/systemprompt-agent) |
| App | [`systemprompt-runtime`](https://docs.rs/systemprompt-runtime) · [`systemprompt-scheduler`](https://docs.rs/systemprompt-scheduler) · [`systemprompt-generator`](https://docs.rs/systemprompt-generator) · [`systemprompt-sync`](https://docs.rs/systemprompt-sync) |
| Entry | [`systemprompt-api`](https://docs.rs/systemprompt-api) · [`systemprompt-cli`](https://docs.rs/systemprompt-cli) |
| Facade | [`systemprompt`](https://docs.rs/systemprompt) |

## Extensions (technical)

Extensions are discovered at **compile time** via the [`inventory`](https://crates.io/crates/inventory) crate — no runtime plugin loading, no `dlopen`. Your code compiles straight into your binary. Typed traits cover the full surface:

| Trait | File | Purpose |
|---|---|---|
| `Extension` | [`crates/shared/extension/src/traits.rs`](crates/shared/extension/src/traits.rs) | Identity, version, dependency metadata |
| `SchemaExtensionTyped` | [`crates/shared/extension/src/typed/schema.rs`](crates/shared/extension/src/typed/schema.rs) | DDL + migrations via `include_str!()` |
| `ApiExtensionTyped` · `ApiExtensionTypedDyn` | [`crates/shared/extension/src/typed/api.rs`](crates/shared/extension/src/typed/api.rs) | Axum route handlers |
| `JobExtensionTyped` | [`crates/shared/extension/src/typed/job.rs`](crates/shared/extension/src/typed/job.rs) | Scheduled and background jobs |
| `ProviderExtensionTyped` | [`crates/shared/extension/src/typed/provider.rs`](crates/shared/extension/src/typed/provider.rs) | Custom LLM / tool / data providers |
| `ConfigExtensionTyped` | [`crates/shared/extension/src/typed/config.rs`](crates/shared/extension/src/typed/config.rs) | Startup config validation |

Registration is a single macro — `register_extension!` lives in [`crates/shared/extension/src/traits.rs`](crates/shared/extension/src/traits.rs) and wraps `inventory::submit!`. Discovery goes through [`ExtensionBuilder<R>`](crates/shared/extension/src/builder.rs) and `TypedExtensionRegistry`.

```toml
[dependencies]
systemprompt = { version = "0.2.1", features = ["full"] }
```

```rust
use systemprompt::extension::prelude::*;

struct MyExtension;

impl Extension for MyExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata::new("my-extension", env!("CARGO_PKG_VERSION"))
    }

    fn schemas(&self) -> Vec<SchemaDefinition> {
        vec![SchemaDefinition::new(
            "my_extension",
            include_str!("../schema/001_init.sql"),
        )]
    }

    fn router(&self) -> Option<ExtensionRouter> { None }
}

register_extension!(MyExtension);
```

## Typed identifiers

**Zero raw-String IDs.** Every identifier that crosses a boundary is a newtype in [`crates/shared/identifiers`](crates/shared/identifiers/src/lib.rs) — the compiler prevents passing a `UserId` where an `AgentId` is expected.

`UserId` · `SessionId` · `TraceId` · `ContextId` · `TaskId` · `AgentId` · `TenantId` · `McpServerId` · `McpExecutionId` · `AiRequestId` · `PluginId` · `SkillId` · `ArtifactId` · `FileId` · `ContentId` · `MessageId` · `TokenId` · `ClientId` · `RoleId` · `ProfileName` · `Email` · `ValidatedUrl` · `ValidatedFilePath`

## Database & repositories

Services call repositories, repositories issue SQL. All queries go through **compile-time verified macros** — `sqlx::query!()`, `sqlx::query_as!()`, `sqlx::query_scalar!()`. No unverified `sqlx::query()`.

DDL lives in `{crate}/schema/*.sql` and is embedded with `include_str!()` from `extension.rs`. The generic entity/repository traits live in [`crates/infra/database/src/repository/entity.rs`](crates/infra/database/src/repository/entity.rs) (`Entity`, `GenericRepository<E>`).

```rust
use systemprompt_database::DbPool;
use systemprompt_identifiers::UserId;

pub struct UserRepository { pool: DbPool }

impl UserRepository {
    pub async fn find_by_id(&self, id: &UserId) -> Result<Option<User>> {
        sqlx::query_as!(User, "SELECT * FROM users WHERE id = $1", id.as_str())
            .fetch_optional(self.pool.as_ref())
            .await
            .map_err(Into::into)
    }
}
```

## Facade crate & feature flags

Pull in only what you need through the `systemprompt` facade.

| Feature | Includes |
|---|---|
| `core` *(default)* | traits · models · identifiers · extension · template-provider |
| `database` | SQLx-backed `DbPool` |
| `api` | HTTP server, runtime, Axum (requires `core` + `database`) |
| `cli` | CLI entry point |
| `runtime` | Extension runtime builder (requires `cli`) |
| `mcp` | `rmcp` macros |
| `sync` | Cloud synchronization |
| `cloud` | Cloud API client, credentials, OAuth |
| `test-utils` | Credential fixtures (requires `cloud`) |
| `full` | Everything: API + MCP + sync + cloud + CLI + all domain crates |

```toml
# Embedded library usage
systemprompt = { version = "0.2.1", features = ["core", "database"] }

# Building a product binary
systemprompt = { version = "0.2.1", features = ["full"] }
```

```rust
use systemprompt::prelude::*;
use systemprompt::database::DbPool;
```

## Performance

Sub-5 ms governance overhead, benchmarked. Each request performs JWT validation, scope resolution, three rule evaluations, and an async database write.

- **p50 < 5 ms**
- **p99 < 12 ms**
- **200 concurrent governance requests**
- Zero GC pauses — hundreds of concurrent developers on a single instance

Numbers measured on the author's laptop. Reproduce with `./demo/performance/02-benchmark.sh` in the template. Full results and a live load test: [systemprompt.io/features/demo](https://systemprompt.io/features/demo).

## Quick Start

**Evaluation path** — you get 40+ runnable demos:

```bash
gh repo create my-eval --template systempromptio/systemprompt-template --clone
cd my-eval
just build
just setup-local <anthropic-or-openai-or-gemini-key>
just start
```

Open **http://localhost:8080**, point Claude Code / Claude Desktop at it, and walk through [`demo/`](https://github.com/systempromptio/systemprompt-template/tree/main/demo). Prerequisites: Rust 1.75+, [`just`](https://just.systems), Docker, `jq`, `yq`, ports `8080` and `5432` free.

**Library path** — add the facade to your own Rust workspace:

```toml
[dependencies]
systemprompt = { version = "0.2.1", features = ["full"] }
```

See [Extensions](#extensions-technical) for the compile-time plugin model.

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. **Production use requires a commercial license.** Each version converts to Apache 2.0 four years after publication.

See [LICENSE](LICENSE) for the full terms. Licensing enquiries: [ed@systemprompt.io](mailto:ed@systemprompt.io).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt)** · **[docs.rs](https://docs.rs/systemprompt)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Own how your organization uses AI. Every interaction governed and provable.</sub>

</div>
