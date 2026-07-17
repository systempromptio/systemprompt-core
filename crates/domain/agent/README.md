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

# systemprompt-agent

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-agent.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/domain-agent.svg">
    <img alt="systemprompt-agent terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-agent.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-agent.svg?style=flat-square)](https://crates.io/crates/systemprompt-agent)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-agent?style=flat-square)](https://docs.rs/systemprompt-agent)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Agents that answer to you, running in your process. An Agent-to-Agent (A2A) protocol server with task lifecycle, discovery, and SSE streaming, where every agent runs as a governed subprocess and every tool call passes through the same audited path.

**Layer**: Domain — business-logic modules built on `shared/*` and `infra/*`. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Capabilities** · [Closed-Loop Agents](https://systemprompt.io/features/closed-loop-agents)

This crate implements the Agent-to-Agent (A2A) protocol and exposes:

- **A2A Server**: JSON-RPC 2.0 message handling with SSE streaming
- **Agent Orchestration**: Subprocess lifecycle, health monitoring, port allocation, state reconciliation
- **Context Management**: Conversation contexts with task, message, and artifact persistence
- **Skill Service**: Skill loading and per-request injection into agent prompts
- **MCP Integration**: Tool execution results transformed into A2A artifacts
- **Registry**: Agent card discovery and security metadata

HTTP routing lives outside this crate. API consumers compose `AgentHandlerState` and `AgentServer` from the `services::a2a_server` module into their own Axum router (typically in `systemprompt-api`).

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                      Services Layer                          │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────┐   │
│  │ A2A Server  │  │ Orchestrator │  │ External Integr.  │   │
│  │  - auth     │  │  - lifecycle │  │  - webhooks       │   │
│  │  - handlers │  │  - monitor   │  │                   │   │
│  │  - processing│ │  - port mgr  │  ├───────────────────┤   │
│  │  - streaming│  │  - reconciler│  │       MCP         │   │
│  └─────────────┘  └──────────────┘  │  - tool results   │   │
│  ┌─────────────┐  ┌──────────────┐  │  - artifact xform │   │
│  │   Context   │  │   Registry   │  └───────────────────┘   │
│  │   Service   │  │  (cards +    │  ┌───────────────────┐   │
│  │             │  │   skills +   │  │     Skills        │   │
│  │             │  │   security)  │  │  - SkillService   │   │
│  └─────────────┘  └──────────────┘  │  - SkillInjector  │   │
│                                     └───────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Repository Layer (SQLX)                   │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────┐   │
│  │    Task     │  │   Context    │  │     Content       │   │
│  │  - queries  │  │  - messages  │  │   - artifacts     │   │
│  │  - mutations│  │  - parts     │  │   - push notif    │   │
│  │  - constructr│ │  - notifs    │  │                   │   │
│  └─────────────┘  └──────────────┘  └───────────────────┘   │
│  ┌─────────────┐  ┌──────────────┐                          │
│  │  Execution  │  │ AgentService │                          │
│  │  steps      │  │              │                          │
│  └─────────────┘  └──────────────┘                          │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Models Layer                            │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────┐   │
│  │    A2A      │  │   Domain     │  │      Web          │   │
│  │  - protocol │  │   - context  │  │   - validation    │   │
│  │  - jsonrpc  │  │   - runtime  │  │   - queries       │   │
│  │  - status   │  │   - rows     │  │   - card input    │   │
│  └─────────────┘  └──────────────┘  └───────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Usage

```toml
[dependencies]
systemprompt-agent = "0.21"
```

## Module Layout

| Module | Purpose |
|--------|---------|
| `models/` | A2A protocol types (JSON-RPC envelopes, `Task`, `Message`, `Artifact`, `AgentCard`, streaming events) plus web request/validation types. |
| `repository/` | Compile-time-verified persistence for agent services, contexts and messages, execution steps, tasks, and artifacts. |
| `services/a2a_server/` | The A2A protocol server: request handlers, AI-execution processing, SSE streaming, JWT auth, and JSON-RPC error mapping. |
| `services/agent_orchestration/` | Agent subprocess lifecycle: orchestrator, state machine, health monitor, port manager, reconciler, and event bus. |
| `services/config_authoring/` | Programmatic editing of agent configuration. |
| `services/mcp/` | MCP tool integration: tool-result handling and transformation into A2A artifacts. |
| `services/registry/` | Agent registry: card loading, security metadata, and skill registration. |
| `services/skills/` | Skill management and injection (`SkillService`, `SkillInjector`). |
| `services/external_integrations/` | Outbound integrations such as webhook delivery. |
| `services/` (files) | `ContextService`, `MessageService`, `ExecutionTrackingService`, `ArtifactPublishingService`, and the `ContextProvider`/`AgentRegistryProvider` trait impls. |

## Schemas

Schemas live in `schema/` and are registered via `AgentExtension`:

| File | Purpose |
|------|---------|
| `agent_tasks.sql` | Task records |
| `artifact_parts.sql` | Artifact part rows |
| `context_agents.sql` | Context-to-agent associations |
| `context_notifications.sql` | Per-context push notification config |
| `message_parts.sql` | Message part rows |
| `services.sql` | Service registry |
| `task_artifacts.sql` | Task-artifact links |
| `task_execution_steps.sql` | Per-step execution audit log |
| `task_messages.sql` | Task message history |
| `task_push_notification_configs.sql` | Per-task push notification config |
| `user_contexts.sql` | User conversation contexts |
| `user_session_analytics.sql` | Session analytics view |

Versioned migrations live in `schema/migrations/`, discovered at build time by the crate's `build.rs` and returned through the `extension_migrations!` macro. Schema DDL (`schema/*.sql`) is for first-install table creation; migrations carry subsequent state transitions.

## Key Components

### A2A Protocol Server (`services/a2a_server/`)

Implements the Agent-to-Agent protocol specification:

| Component | Purpose |
|-----------|---------|
| `server.rs` | `AgentServer` construction |
| `standalone.rs` | Entry point for spawned agent subprocesses |
| `handlers/` | Card, push notification config, and message request handlers |
| `processing/` | AI execution, conversation/message handling, strategy selection, task building |
| `streaming/` | SSE event loop, broadcast, webhook client |
| `auth/` | JWT validation middleware |
| `errors/` | JSON-RPC error mapping |

### Agent Orchestration (`services/agent_orchestration/`)

Manages agent subprocess lifecycle:

| Component | Purpose |
|-----------|---------|
| `orchestrator/` | `AgentOrchestrator`: start, stop, restart, status, cleanup |
| `lifecycle/` | State machine + post-start verification |
| `monitor.rs` | Health probing via agent cards |
| `port_manager/` | Dynamic port allocation with reachability probes |
| `process/` | Subprocess command builder + signal handling |
| `reconciler.rs` | Reconcile DB state with running processes |
| `event_bus.rs` / `events.rs` | `AgentEventBus` + `AgentEvent` |

### Context Management

Conversation context with full history:

| Component | Purpose |
|-----------|---------|
| `repository/context/` | `ContextRepository` (CRUD + message persistence) |
| `services/context.rs` | `ContextService` (load conversation history for AI) |
| `services/context_provider.rs` | `ContextProvider` trait impl |

## Public Exports

```rust
pub use models::a2a::{
    A2aJsonRpcRequest, A2aRequestParams, A2aResponse, AgentCapabilities,
    AgentCard, AgentInterface, AgentProvider, AgentSkill, Artifact,
    DataPart, Message, MessageSendParams, Part, SecurityScheme, Task,
    TaskIdParams, TaskQueryParams, TaskState, TaskStatus, TextPart,
    TransportProtocol,
};

pub use error::{
    AgentError, AgentResult, ArtifactError, ContextError,
    ProtocolError, RowParseError, TaskError,
};

pub use services::{
    AgentEvent, AgentEventBus, AgentHandlerState, AgentOrchestrator,
    AgentServer, AgentStatus, ContextService, SkillService,
};

pub use repository::content::ArtifactRepository;
pub use extension::AgentExtension;
pub use state::AgentState;

pub const A2A_PROTOCOL_VERSION: &str = "0.3.0";
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-models` | Shared domain types |
| `systemprompt-traits` | `ContextProvider`, `AgentRegistryProvider`, related traits |
| `systemprompt-identifiers` | Typed IDs (`TaskId`, `ContextId`, `UserId`, `SessionId`, …) |
| `systemprompt-extension` | `Extension` trait + registration |
| `systemprompt-config` | Profile + runtime config |
| `systemprompt-database` | SQLX pool and helpers |
| `systemprompt-events` | Event bus + SSE plumbing |
| `systemprompt-logging` | `tracing` setup helpers |
| `systemprompt-loader` | Filesystem discovery helpers |
| `systemprompt-security` | JWT / signing primitives |
| `rmcp` | MCP protocol client |

## Features

The crate is feature-flag-free; functionality is unconditional. The facade crate `systemprompt` gates inclusion via its `full` feature (there is no standalone `agent` feature).

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-agent)** · **[docs.rs](https://docs.rs/systemprompt-agent)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
