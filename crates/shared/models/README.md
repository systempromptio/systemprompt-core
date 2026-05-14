<div align="center">

<picture>
  <source media="(prefers-color-scheme: dark)" srcset="https://systemprompt.io/files/images/logo.svg">
  <source media="(prefers-color-scheme: light)" srcset="https://systemprompt.io/files/images/logo-dark.svg">
  <img src="https://systemprompt.io/files/images/logo.svg" alt="systemprompt.io" width="180">
</picture>

### Production infrastructure for AI agents

[**Website**](https://systemprompt.io) · [**Documentation**](https://systemprompt.io/documentation/) · [**Guides**](https://systemprompt.io/guides) · [**Core**](https://github.com/systempromptio/systemprompt-core) · [**Template**](https://github.com/systempromptio/systemprompt-template) · [**Discord**](https://discord.gg/wkAbSuPWpr)

</div>

---

# systemprompt-models

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/00-overview.svg">
    <img alt="systemprompt-models — systemprompt-core workspace" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/00-overview.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-models.svg?style=flat-square)](https://crates.io/crates/systemprompt-models)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-models?style=flat-square)](https://docs.rs/systemprompt-models)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Foundation data models for systemprompt.io. Plain DTOs, on-disk configuration, protocol shapes (A2A, AG-UI, MCP), and the typed error enums every public function in the workspace returns. Consumed by every other layer (`infra`, `domain`, `app`, `entry`).

**Layer**: Shared — no dependencies on other systemprompt layers. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Installation

```toml
[dependencies]
systemprompt-models = "0.9"
```

## Module Map

| Module | Purpose |
|--------|---------|
| `a2a` | A2A protocol: agent card, message, task, transport, security scheme types. |
| `admin` | Admin dashboard DTOs (analytics, traffic, log entries, user metrics). |
| `agui` | AG-UI streaming event protocol (events, payloads, builders). |
| `ai` | LLM request/response shapes, `AiProvider` trait, streaming chunks, tool execution. |
| `api` | Public HTTP envelopes, pagination, error model, cloud API DTOs. |
| `artifacts` | Typed tool-result artifacts (chart, table, image, cli, …) and conversion. |
| `auth` | Authenticated user, base roles, JWT audience, PKCE, grant types. |
| `bridge` | Cowork desktop bridge manifest types. |
| `config` | Global `Config` singleton assembled from profile + secrets. |
| `content`, `content_config` | Published content metadata and on-disk content routing. |
| `errors` | `thiserror`-derived `CoreError`, `RepositoryError`, `ServiceError`. |
| `events` | Analytics, A2A, context, and system event envelopes. |
| `execution` | `RequestContext`, `ExecutionStep`, planned-tool bookkeeping. |
| `extension` | Extension framework manifest and discovery types. |
| `gateway_hash` | Stable hashing helpers for gateway-derived identifiers. |
| `macros` | Crate-internal repository helper macros. |
| `mcp` | MCP server/registry config, deployment, auth state, provider traits. |
| `modules` | API path constants, CLI paths, service category resolution. |
| `net` | Network-layer value objects (ports, hosts). |
| `oauth` | OAuth client and server configuration shapes. |
| `paths` | Well-known directory layout helpers (`AppPaths`, `StoragePaths`, …). |
| `profile` | On-disk profile, security, server, cloud, database, paths configuration. |
| `repository` | `ServiceLifecycle` trait, `ServiceRecord`, `WhereClause` query builder. |
| `routing` | Request routing classification (`RouteClassifier`, `ApiCategory`). |
| `secrets` | Secrets document model and parsing. |
| `services` | Services manifest: agents, plugins, hooks, MCP, skills, scheduler, marketplace. |
| `text`, `time_format` | Small text and timestamp formatting helpers. |
| `users` | Public user and session summaries. |
| `validators` | Startup configuration validation passes. |

## Error Model

Three `thiserror` enums layered from database to HTTP:

```text
RepositoryError → ServiceError → ApiError → HTTP Response
```

```rust
use systemprompt_models::{RepositoryError, ServiceError, ApiError};

let repo_err = RepositoryError::NotFound("user-123".to_string());
let svc_err: ServiceError = repo_err.into();
let api_err: ApiError = svc_err.into();
```

`anyhow::Error` is never used in a public signature in this crate.

## Request Context

```rust
use systemprompt_models::{RequestContext, SessionId, TraceId, UserId, ContextId};

let ctx = RequestContext {
    session_id: SessionId::generate(),
    trace_id: TraceId::generate(),
    user_id: UserId::new("user-789"),
    context_id: ContextId::generate(),
    task_id: None,
    ai_tool_call_id: None,
    client_id: None,
    auth_token: None,
    user: None,
    start_time: std::time::Instant::now(),
    user_type: Default::default(),
};
```

## Repository Helpers

```rust
use systemprompt_models::WhereClause;

let (clause, params) = WhereClause::new()
    .eq("status", "active")
    .is_not_null("pid")
    .build();
```

`ServiceLifecycle` provides the common `get_running_services` / `mark_crashed` / `update_status` surface implemented by repositories that supervise long-running processes.

## Feature Flags

| Feature | Default | Description |
|---------|---------|-------------|
| `web` | off | `axum::IntoResponse` impls for the API envelopes. |

## Dependencies

- `serde`, `serde_json`, `serde_yaml` — serialization
- `thiserror` — error enums
- `chrono`, `uuid` — common types
- `schemars`, `validator`, `regex` — schema generation and validation
- `rmcp` — MCP protocol types
- `axum` — optional, with the `web` feature
- `systemprompt-traits`, `systemprompt-identifiers`, `systemprompt-extension`, `systemprompt-provider-contracts` — shared layer siblings

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-models)** · **[docs.rs](https://docs.rs/systemprompt-models)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Shared layer · Own how your organization uses AI.</sub>

</div>
