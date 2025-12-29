# Agent Module

A2A protocol server/client, agent orchestration, and agent business logic.

## Directory Structure

```
src/
├── lib.rs                    # Module exports
├── error.rs                  # AgentError, ContextError, TaskError, etc.
├── api/                      # HTTP API layer
│   └── routes/
│       ├── artifacts.rs      # Artifact CRUD endpoints
│       ├── registry.rs       # Agent registry endpoints
│       ├── tasks.rs          # Task management endpoints
│       └── contexts/         # Context management
│           ├── create_context.rs
│           ├── get_context.rs
│           ├── list_contexts.rs
│           ├── update_context.rs
│           ├── delete_context.rs
│           ├── events.rs     # SSE streaming
│           ├── notifications/ # Push notifications
│           └── webhook/      # Internal webhooks
│
├── models/                   # Data structures
│   ├── agent_info.rs         # AgentInfo
│   ├── context.rs            # Context types
│   ├── database_rows.rs      # DB row mappings
│   ├── runtime.rs            # AgentRuntimeInfo
│   ├── skill.rs              # Skill types
│   ├── web/                  # Web API types (validation, query params)
│   └── a2a/                  # A2A Protocol types
│       ├── jsonrpc.rs        # JSON-RPC 2.0
│       ├── protocol.rs       # Task, Message, Artifact
│       └── service_status.rs # Service status extension
│
├── repository/               # Database access
│   ├── agent_service/        # Agent service repo
│   ├── content/              # Artifact, skill, push notification repos
│   ├── context/              # Context repo with message queries
│   ├── execution/            # Execution step tracking repo
│   └── task/                 # Task repo (queries, mutations, constructor)
│
└── services/                 # Business logic
    ├── context_provider.rs   # Implements ContextProvider trait
    ├── registry_provider.rs  # Implements AgentRegistryProvider trait
    ├── message.rs            # Message persistence
    ├── execution_tracking.rs # Step tracking
    ├── registry.rs           # Agent registry
    │
    ├── a2a_server/           # A2A Server
    │   ├── server.rs
    │   ├── auth/             # JWT validation, middleware
    │   ├── errors/           # JSON-RPC error handling
    │   ├── handlers/         # Request handlers (card, push config, streaming)
    │   ├── processing/       # Message handling, AI execution, strategies
    │   │   ├── artifact/     # Artifact building from tool results
    │   │   ├── message/      # Stream processing
    │   │   └── strategies/   # Execution strategies (standard, planned)
    │   └── streaming/        # SSE event streaming
    │
    ├── agent_orchestration/  # Lifecycle management
    │   ├── orchestrator/     # Start/stop/restart agents
    │   ├── database.rs       # State persistence
    │   ├── lifecycle.rs      # Lifecycle state machine
    │   ├── monitor.rs        # Health checking
    │   ├── port_manager.rs   # Port allocation
    │   └── reconciler.rs     # State consistency
    │
    ├── external_integrations/
    │   ├── mcp/              # MCP type re-exports
    │   └── webhook/          # Event broadcasting
    │
    ├── mcp/                  # MCP result processing
    │   ├── task_helper.rs
    │   └── artifact_transformer/ # Transform tool results to artifacts
    │
    ├── shared/               # Shared utilities (auth, config, resilience)
    │
    └── skills/               # Skill management (loading, injection, ingestion)
```

## Key Files

| File | Purpose |
|------|---------|
| `lib.rs` | Public exports: A2A types, errors, services |
| `error.rs` | Error types using thiserror |
| `api/routes/` | Axum route handlers |
| `services/a2a_server/server.rs` | A2A protocol server |
| `services/agent_orchestration/orchestrator/` | Agent lifecycle management |
| `repository/task/` | Task CRUD with queries/mutations separation |

## Dependencies

| Crate | Purpose |
|-------|---------|
| systemprompt-core-database | Database pool |
| systemprompt-core-oauth | OAuth validation |
| systemprompt-core-users | User services |
| systemprompt-core-logging | Logging infrastructure |
| systemprompt-core-mcp | MCP protocol (boundary violation - should use traits) |
| systemprompt-core-system | Core system types |
| systemprompt-models | Shared domain types |
| systemprompt-traits | Trait definitions |
| systemprompt-identifiers | Typed identifiers |
