<div align="center">
  <a href="https://systemprompt.io">
    <img src="https://systemprompt.io/logo.svg" alt="systemprompt.io" width="150" />
  </a>
  <p><strong>Production infrastructure for AI agents</strong></p>
  <p><a href="https://systemprompt.io">systemprompt.io</a> • <a href="https://github.com/systempromptio/systemprompt">GitHub</a> • <a href="https://docs.systemprompt.io">Documentation</a></p>
</div>

---


# systemprompt-mcp

Core MCP (Model Context Protocol) functionality for systemprompt.io OS.

[![Crates.io](https://img.shields.io/crates/v/systemprompt-mcp.svg)](https://crates.io/crates/systemprompt-mcp)
[![Documentation](https://docs.rs/systemprompt-mcp/badge.svg)](https://docs.rs/systemprompt-mcp)
[![License: FSL-1.1-ALv2](https://img.shields.io/badge/License-FSL--1.1--ALv2-blue.svg)](https://github.com/systempromptio/systemprompt/blob/main/LICENSE)

## Overview

**Part of the Domain layer in the systemprompt.io architecture.**

MCP (Model Context Protocol) server lifecycle management module.

## Installation

Add to your `Cargo.toml`:

```toml
[dependencies]
systemprompt-mcp = "0.0.1"
```

## Trait Implementations

| Trait | Implementation | Location |
|-------|----------------|----------|
| `ToolProvider` | `McpToolProvider` | `services/tool_provider.rs:135` |
| `McpRegistry` | `RegistryManager` | `services/registry/trait_impl.rs` |
| `McpToolProvider` | `RegistryManager` | `services/registry/trait_impl.rs` |
| `McpDeploymentProvider` | `McpDeploymentProviderImpl` | `services/registry/trait_impl.rs` |

## Structure

```
src/
├── lib.rs                          # Crate entry, router creation, re-exports
├── orchestration/
│   ├── mod.rs                      # Orchestration module exports
│   ├── loader.rs                   # McpToolLoader - batch tool loading with permissions
│   ├── state.rs                    # ServiceStateManager - service state queries
│   └── models.rs                   # McpServiceState, ServerStatus, SkillLoadingResult
├── api/
│   ├── mod.rs                      # API router definitions
│   └── routes/
│       ├── mod.rs                  # Route aggregation
│       └── registry.rs             # Registry query endpoints
├── cli/
│   ├── mod.rs                      # CLI command exports
│   ├── commands/mod.rs             # CLI command handlers
│   └── display.rs                  # Terminal output formatting
├── middleware/
│   ├── mod.rs                      # Middleware exports
│   ├── rbac.rs                     # Role-based access control
│   └── session_manager.rs          # MCP session management
├── models/
│   └── mod.rs                      # ExecutionStatus, ValidationResultType, ToolExecution
├── repository/
│   ├── mod.rs                      # Repository exports
│   └── tool_usage/
│       └── mod.rs                  # Tool execution persistence
└── services/
    ├── mod.rs                      # Service traits, manager exports
    ├── client/
    │   ├── mod.rs                  # MCP client
    │   ├── http_client_with_context.rs  # HTTP client with context
    │   ├── types.rs                # Client types
    │   └── validation.rs           # Connection validation
    ├── database/
    │   ├── mod.rs                  # Database manager
    │   ├── state.rs                # State operations
    │   └── sync.rs                 # State synchronization
    ├── deployment/
    │   └── mod.rs                  # Deployment configuration
    ├── lifecycle/
    │   ├── mod.rs                  # Lifecycle manager
    │   ├── health.rs               # Health checks
    │   ├── restart.rs              # Server restart
    │   ├── shutdown.rs             # Graceful shutdown
    │   └── startup.rs              # Server startup
    ├── monitoring/
    │   ├── mod.rs                  # Monitoring manager
    │   ├── health.rs               # HealthStatus, health execution
    │   ├── proxy_health.rs         # Proxy health monitoring
    │   └── status.rs               # Service status reporting
    ├── network/
    │   ├── mod.rs                  # Network manager
    │   ├── port_manager.rs         # Port allocation
    │   ├── proxy.rs                # HTTP proxy
    │   └── routing.rs              # Router and CORS
    ├── orchestrator/
    │   ├── mod.rs                  # McpOrchestrator coordinator
    │   ├── daemon.rs               # Background daemon
    │   ├── event_bus.rs            # Pub/sub events
    │   ├── events.rs               # McpEvent definitions
    │   ├── process_cleanup.rs      # Stale process cleanup
    │   ├── reconciliation.rs       # State reconciliation
    │   ├── service_validation.rs   # Service validation
    │   └── handlers/
    │       ├── mod.rs              # EventHandler trait
    │       ├── database_sync.rs    # DB sync handling
    │       ├── health_check.rs     # Health check handling
    │       ├── lifecycle.rs        # Lifecycle handling
    │       └── monitoring.rs       # Monitoring handling
    ├── process/
    │   ├── mod.rs                  # Process manager
    │   ├── cleanup.rs              # Process termination
    │   ├── monitor.rs              # Process monitoring
    │   ├── pid_manager.rs          # PID tracking
    │   └── spawner.rs              # Process spawning
    ├── registry/
    │   ├── mod.rs                  # Registry manager
    │   ├── manager.rs              # Registry implementation
    │   ├── trait_impl.rs           # McpRegistry, McpToolProvider trait impls
    │   └── validator.rs            # Registry validation
    ├── tool_provider.rs            # ToolProvider implementation
    └── schema/
        ├── mod.rs                  # Schema service
        ├── loader.rs               # Schema loading
        └── validator.rs            # Schema validation
```

## Database

Schema: `schema/mcp_tool_executions.sql`

Uses `ToolUsageRepository` for tool execution tracking.

## License

FSL-1.1-ALv2 - See [LICENSE](https://github.com/systempromptio/systemprompt/blob/main/LICENSE) for details.
