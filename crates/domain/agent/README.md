<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://systemprompt.io/documentation">Documentation</a></p>
</div>

---


# systemprompt-agent

Core Agent protocol module for systemprompt.io.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-agent.svg)](https://crates.io/crates/systemprompt-agent)
[![Documentation](https://docs.rs/systemprompt-agent/badge.svg)](https://docs.rs/systemprompt-agent)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](https://github.com/systempromptio/systemprompt/blob/main/LICENSE)

## Overview

**Part of the Domain layer in the systemprompt.io architecture.**

A2A protocol server/client, agent orchestration, and agent business logic.

This crate implements the Agent-to-Agent (A2A) protocol, providing:

- **A2A Server**: JSON-RPC 2.0 based protocol for agent communication
- **Agent Orchestration**: Lifecycle management for spawning and monitoring agents
- **Context Management**: Conversation contexts with task and artifact tracking
- **Skill System**: Dynamic skill loading and injection for agents
- **MCP Integration**: Tool execution via Model Context Protocol

## Architecture

```
┌─────────────────────────────────────────────────────────────┐
│                        API Layer                             │
│  (routes for contexts, tasks, artifacts, registry, webhook)  │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Services Layer                          │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────┐   │
│  │ A2A Server  │  │ Orchestrator │  │ External Integr.  │   │
│  │  - auth     │  │  - lifecycle │  │  - MCP tools      │   │
│  │  - handlers │  │  - monitor   │  │  - webhooks       │   │
│  │  - stream   │  │  - port mgmt │  │                   │   │
│  │  - process  │  │  - reconcile │  │                   │   │
│  └─────────────┘  └──────────────┘  └───────────────────┘   │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────┐   │
│  │   Context   │  │   Registry   │  │     Skills        │   │
│  │   Service   │  │   Service    │  │   - ingestion     │   │
│  │             │  │              │  │   - injection     │   │
│  └─────────────┘  └──────────────┘  └───────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                    Repository Layer                          │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────┐   │
│  │    Task     │  │   Context    │  │     Content       │   │
│  │  - queries  │  │  - messages  │  │   - artifacts     │   │
│  │  - mutations│  │  - parts     │  │   - skills        │   │
│  │  - construct│  │  - persist   │  │   - push notif    │   │
│  └─────────────┘  └──────────────┘  └───────────────────┘   │
└─────────────────────────────────────────────────────────────┘
                              │
                              ▼
┌─────────────────────────────────────────────────────────────┐
│                      Models Layer                            │
│  ┌─────────────┐  ┌──────────────┐  ┌───────────────────┐   │
│  │    A2A      │  │   Context    │  │      Web          │   │
│  │  - protocol │  │   - events   │  │   - validation    │   │
│  │  - jsonrpc  │  │   - requests │  │   - queries       │   │
│  │  - status   │  │              │  │   - card input    │   │
│  └─────────────┘  └──────────────┘  └───────────────────┘   │
└─────────────────────────────────────────────────────────────┘
```

## Directory Structure

```
src/
├── lib.rs                          # Crate root, public exports
├── error.rs                        # Domain error types (thiserror)
│
├── api/                            # HTTP API layer (Axum routes)
│   ├── mod.rs                      # Router constructors
│   └── routes/
│       ├── mod.rs
│       ├── artifacts.rs            # GET /artifacts, /artifacts/:id
│       ├── registry.rs             # GET /registry (agent cards)
│       ├── tasks.rs                # GET/DELETE /tasks, /tasks/:id
│       └── contexts/
│           ├── mod.rs              # Context router
│           ├── create_context.rs   # POST /contexts
│           ├── get_context.rs      # GET /contexts/:id
│           ├── list_contexts.rs    # GET /contexts
│           ├── update_context.rs   # PUT /contexts/:id
│           ├── delete_context.rs   # DELETE /contexts/:id
│           ├── events.rs           # POST /contexts/:id/events
│           ├── notifications/
│           │   └── mod.rs          # Push notification handlers
│           └── webhook/
│               ├── mod.rs
│               ├── broadcast_handlers.rs
│               ├── context_broadcast.rs
│               ├── event_loader.rs
│               ├── types.rs
│               └── validation.rs
│
├── models/                         # Data structures
│   ├── mod.rs                      # Re-exports
│   ├── agent_info.rs               # AgentInfo
│   ├── context.rs                  # UserContext, ContextStateEvent
│   ├── database_rows.rs            # DB row mappings
│   ├── external_integrations.rs    # Integration types
│   ├── runtime.rs                  # AgentRuntimeInfo
│   ├── skill.rs                    # Skill, SkillMetadata
│   ├── a2a/
│   │   ├── mod.rs                  # A2A type re-exports
│   │   ├── jsonrpc.rs              # JSON-RPC 2.0 types
│   │   ├── protocol.rs             # Task, Message, Artifact, Part
│   │   └── service_status.rs       # Service status extension
│   └── web/
│       ├── mod.rs
│       ├── card_input.rs           # Agent card input validation
│       ├── create_agent.rs         # CreateAgentRequest
│       ├── discovery.rs            # Agent discovery types
│       ├── query.rs                # Query parameters
│       ├── update_agent.rs         # UpdateAgentRequest
│       └── validation.rs           # URL validation
│
├── repository/                     # Database access (SQLX)
│   ├── mod.rs                      # Repository trait, A2ARepositories
│   ├── agent_service/
│   │   └── mod.rs                  # AgentServiceRepository
│   ├── content/
│   │   ├── mod.rs
│   │   ├── artifact.rs             # ArtifactRepository
│   │   ├── push_notification.rs    # PushNotificationConfigRepository
│   │   └── skill.rs                # SkillRepository
│   ├── context/
│   │   ├── mod.rs                  # ContextRepository
│   │   └── message/
│   │       ├── mod.rs
│   │       ├── parts.rs            # Message part handling
│   │       ├── persistence.rs      # Message persistence
│   │       └── queries.rs          # Message queries
│   ├── execution/
│   │   └── mod.rs                  # ExecutionStepRepository
│   └── task/
│       ├── mod.rs                  # TaskRepository
│       ├── mutations.rs            # Task create/update
│       ├── queries.rs              # Task queries
│       └── constructor/
│           ├── mod.rs              # TaskConstructor
│           ├── batch.rs            # Batch task construction
│           ├── batch_queries.rs    # Batch query helpers
│           ├── converters.rs       # Row to model converters
│           └── single.rs           # Single task construction
│
└── services/                       # Business logic
    ├── mod.rs                      # Service re-exports
    ├── artifact_publishing.rs      # Artifact publishing service
    ├── context.rs                  # ContextService (history loading)
    ├── context_provider.rs         # ContextProvider trait impl
    ├── execution_tracking.rs       # ExecutionTrackingService
    ├── message.rs                  # MessageService
    ├── registry.rs                 # AgentRegistry (config loading)
    ├── registry_provider.rs        # AgentRegistryProvider trait impl
    │
    ├── a2a_server/                 # A2A Protocol Server
    │   ├── mod.rs
    │   ├── server.rs               # Main server setup
    │   ├── standalone.rs           # Standalone agent runner
    │   ├── auth/
    │   │   ├── mod.rs
    │   │   ├── middleware.rs       # Auth middleware
    │   │   ├── types.rs            # Auth types
    │   │   └── validation.rs       # JWT validation
    │   ├── errors/
    │   │   ├── mod.rs
    │   │   └── jsonrpc.rs          # JSON-RPC error codes
    │   ├── handlers/
    │   │   ├── mod.rs              # AgentHandlerState
    │   │   ├── card.rs             # Agent card handler
    │   │   ├── push_notification_config.rs
    │   │   ├── state.rs            # Handler state management
    │   │   └── request/
    │   │       ├── mod.rs          # Request routing
    │   │       ├── non_streaming.rs
    │   │       ├── streaming.rs
    │   │       └── validation.rs
    │   ├── processing/
    │   │   ├── mod.rs
    │   │   ├── ai_executor.rs      # AI request execution
    │   │   ├── conversation_service.rs
    │   │   ├── message_validation.rs
    │   │   ├── persistence_service.rs
    │   │   ├── task_builder.rs     # Task construction
    │   │   ├── artifact/
    │   │   │   └── mod.rs          # Artifact building
    │   │   ├── message/
    │   │   │   ├── mod.rs
    │   │   │   ├── message_handler.rs
    │   │   │   ├── persistence.rs
    │   │   │   └── stream_processor.rs
    │   │   └── strategies/
    │   │       ├── mod.rs          # Strategy pattern
    │   │       ├── planned.rs      # Planned execution
    │   │       ├── plan_executor.rs
    │   │       ├── selector.rs     # Strategy selection
    │   │       ├── standard.rs     # Standard execution
    │   │       └── tool_executor.rs
    │   └── streaming/
    │       ├── mod.rs
    │       ├── agent_loader.rs     # Agent loading for streaming
    │       ├── broadcast.rs        # Event broadcasting
    │       ├── event_loop.rs       # SSE event loop
    │       ├── initialization.rs   # Stream initialization
    │       ├── messages.rs         # Message formatting
    │       ├── types.rs            # Streaming types
    │       ├── webhook_client.rs   # Webhook client
    │       └── handlers/
    │           ├── mod.rs
    │           ├── completion.rs   # Completion handlers
    │           └── text.rs         # Text handlers
    │
    ├── agent_orchestration/        # Agent Lifecycle Management
    │   ├── mod.rs                  # Exports, OrchestrationError
    │   ├── database.rs             # State persistence
    │   ├── event_bus.rs            # AgentEventBus
    │   ├── events.rs               # AgentEvent types
    │   ├── lifecycle.rs            # State machine
    │   ├── monitor.rs              # Health monitoring
    │   ├── port_manager.rs         # Port allocation
    │   ├── process.rs              # Process spawning
    │   ├── reconciler.rs           # State reconciliation
    │   └── orchestrator/
    │       ├── mod.rs              # AgentOrchestrator
    │       ├── cleanup.rs          # Cleanup operations
    │       ├── daemon.rs           # Daemon management
    │       └── status.rs           # Status queries
    │
    ├── external_integrations/      # External Service Integrations
    │   ├── mod.rs                  # Integration exports
    │   ├── mcp/
    │   │   └── mod.rs              # MCP type re-exports
    │   └── webhook/
    │       ├── mod.rs
    │       └── service.rs          # WebhookService
    │
    ├── mcp/                        # MCP Tool Integration
    │   ├── mod.rs
    │   ├── task_helper.rs          # Task helper functions
    │   ├── tool_result_handler.rs  # Tool result processing
    │   └── artifact_transformer/
    │       ├── mod.rs              # Artifact transformation
    │       ├── metadata_builder.rs
    │       ├── parts_builder.rs
    │       └── type_inference.rs
    │
    ├── shared/                     # Shared Utilities
    │   ├── mod.rs
    │   ├── auth.rs                 # Auth utilities
    │   ├── config.rs               # Agent config types
    │   ├── error.rs                # Shared error types
    │   ├── resilience.rs           # Retry logic
    │   └── slug.rs                 # Slug generation
    │
    └── skills/                     # Skill Management
        ├── mod.rs
        ├── ingestion.rs            # SkillIngestionService
        ├── skill.rs                # SkillService
        └── skill_injector.rs       # Skill injection
```

## Key Components

### A2A Protocol Server

Implements the Agent-to-Agent protocol specification:

| Component | Purpose |
|-----------|---------|
| `server.rs` | Axum server setup with routes |
| `handlers/` | Request handlers for card, push notifications, message send |
| `processing/` | Message processing, AI execution, artifact building |
| `streaming/` | SSE streaming for real-time updates |
| `auth/` | JWT validation and middleware |

### Agent Orchestration

Manages agent process lifecycle:

| Component | Purpose |
|-----------|---------|
| `orchestrator/` | Start, stop, restart agents |
| `lifecycle.rs` | State machine (Starting → Running → Stopping) |
| `monitor.rs` | Health check via agent cards |
| `port_manager.rs` | Dynamic port allocation |
| `reconciler.rs` | Sync DB state with running processes |

### Context Management

Conversation context with full history:

| Component | Purpose |
|-----------|---------|
| `ContextRepository` | CRUD for user contexts |
| `ContextService` | Load conversation history for AI |
| `message/` | Message persistence with parts |

## Public Exports

```rust
pub use models::a2a::{
    A2aJsonRpcRequest, A2aRequestParams, A2aResponse, AgentCapabilities,
    AgentCard, AgentInterface, AgentProvider, AgentSkill, Artifact,
    DataPart, Message, MessageSendParams, Part, SecurityScheme, Task,
    TaskIdParams, TaskQueryParams, TaskState, TaskStatus, TextPart,
    TransportProtocol,
};

pub use error::{AgentError, ArtifactError, ContextError, ProtocolError, TaskError};

pub use services::{
    AgentEvent, AgentEventBus, AgentHandlerState, AgentOrchestrator,
    AgentServer, AgentStatus, ContextService, SkillIngestionService,
    SkillService,
};
```

## Dependencies

| Crate | Purpose |
|-------|---------|
| `systemprompt-models` | Shared domain types |
| `systemprompt-traits` | Trait definitions (ContextProvider, AgentRegistryProvider) |
| `systemprompt-identifiers` | Typed identifiers (TaskId, ContextId, etc.) |
| `systemprompt-runtime` | AppContext, runtime services |
| `systemprompt-database` | Database pool and utilities |
| `systemprompt-mcp` | MCP protocol integration |
| `systemprompt-ai` | AI service for agent execution |
| `systemprompt-events` | Event routing |
| `systemprompt-oauth` | OAuth and JWT handling |

## Features

| Feature | Description |
|---------|-------------|
| `default` | Includes `web` feature |
| `web` | HTTP API routes (Axum, Tower) |
| `cli` | CLI-specific functionality |

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-agent = "0.0.1"
```

## License

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt/blob/main/LICENSE) for details.
