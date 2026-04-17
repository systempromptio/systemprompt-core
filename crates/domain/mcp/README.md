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

# systemprompt-mcp

<div align="center">
  <picture>
    <source media="(prefers-color-scheme: dark)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-mcp.svg">
    <source media="(prefers-color-scheme: light)" srcset="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/light/domain-mcp.svg">
    <img alt="systemprompt-mcp terminal demo" src="https://raw.githubusercontent.com/systempromptio/systemprompt-core/main/assets/readme/terminals/dark/domain-mcp.svg" width="100%">
  </picture>
</div>

[![Crates.io](https://img.shields.io/crates/v/systemprompt-mcp.svg?style=flat-square)](https://crates.io/crates/systemprompt-mcp)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-mcp?style=flat-square)](https://docs.rs/systemprompt-mcp)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Native Model Context Protocol (MCP) implementation for systemprompt.io. Orchestration, per-server OAuth2, RBAC middleware, and tool-call governance — the core of the AI governance pipeline.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Capabilities** · [MCP Governance](https://systemprompt.io/features/mcp-governance)

MCP (Model Context Protocol) server lifecycle management module.

## Usage

```toml
[dependencies]
systemprompt-mcp = "0.2.1"
```

## Architecture

### Trait Implementations

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

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-mcp)** · **[docs.rs](https://docs.rs/systemprompt-mcp)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
