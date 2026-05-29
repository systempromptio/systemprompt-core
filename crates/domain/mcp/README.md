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

Native MCP server lifecycle, orchestration, and governance. Manages MCP process spawning, port allocation, proxy routing, RBAC middleware, schema validation, tool execution, artifact persistence, and UI rendering for the systemprompt.io platform.

## Usage

```toml
[dependencies]
systemprompt-mcp = "0.13.0"
```

## Architecture

### Trait Implementations

| Trait | Implementation | Location |
|-------|----------------|----------|
| `ToolProvider` | `McpToolProvider` | `services/tool_provider/mod.rs` |
| `McpRegistry` | `RegistryManager` | `services/registry/trait_impl.rs` |
| `McpToolProvider` | `RegistryManager` | `services/registry/trait_impl.rs` |
| `McpDeploymentProvider` | `McpDeploymentProviderImpl` | `services/registry/trait_impl.rs` |
| `Extension` | `McpExtension` | `extension.rs` |

## Structure

```
src/
├── lib.rs                          # Crate entry, router creation, re-exports
├── capabilities.rs                 # MCP Apps UI extension helpers
├── error.rs                        # McpDomainError and error types
├── extension.rs                    # McpExtension - schema registration, jobs, routes
├── progress.rs                     # Tool execution progress reporting
├── resources.rs                    # MCP resource definitions
├── response.rs                     # McpResponseBuilder and artifact response shaping
├── schema.rs                       # McpOutputSchema trait + artifact type bindings
├── state.rs                        # Shared crate state
├── tool.rs                         # Tool trait, McpToolExecutor
├── cli/
│   ├── mod.rs                      # CLI command exports
│   └── commands/mod.rs             # CLI command handlers
├── jobs/
│   ├── mod.rs                      # Background job exports
│   └── mcp_session_cleanup.rs      # Stale session cleanup job
├── middleware/
│   ├── mod.rs                      # Middleware exports
│   ├── session_manager.rs          # DatabaseSessionManager
│   ├── rbac.rs                     # RBAC entry point
│   └── rbac/
│       ├── jwt.rs                  # JWT-based RBAC
│       └── proxy.rs                # Proxy-verified identity RBAC
├── models/
│   └── mod.rs                      # ExecutionStatus, ValidationResultType, ToolExecution
├── orchestration/
│   ├── mod.rs                      # Orchestration module exports
│   ├── loader.rs                   # McpToolLoader - batch tool loading
│   ├── state.rs                    # ServiceStateManager - service state queries
│   └── models.rs                   # McpServiceState, ServerStatus, SkillLoadingResult
├── repository/
│   ├── mod.rs                      # Repository exports
│   ├── artifact/mod.rs             # McpArtifactRepository
│   ├── session/mod.rs              # McpSessionRepository
│   └── tool_usage/
│       ├── mod.rs                  # Tool execution persistence
│       └── stats.rs                # Tool usage statistics
└── services/
    ├── mod.rs                      # Service traits, manager exports
    ├── auth.rs                     # Auth helpers
    ├── providers.rs                # Provider trait wiring
    ├── client/
    │   ├── mod.rs                  # MCP client
    │   ├── http_client_with_context.rs  # HTTP client with context propagation
    │   ├── types.rs                # Client types
    │   └── validation.rs           # Connection validation
    ├── database/
    │   ├── mod.rs                  # Database manager
    │   ├── state.rs                # State operations
    │   └── sync.rs                 # State synchronisation
    ├── deployment/mod.rs           # Deployment configuration
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
    │   ├── event_bus.rs            # Pub/sub event bus
    │   ├── events.rs               # McpEvent definitions
    │   ├── lifecycle_ops.rs        # Lifecycle operations
    │   ├── process_cleanup.rs      # Stale process cleanup
    │   ├── reconciliation.rs       # State reconciliation
    │   ├── schema_sync.rs          # Schema synchronisation
    │   ├── server_startup.rs       # Server startup orchestration
    │   ├── service_validation.rs   # Service validation
    │   ├── target_resolution.rs    # Server target routing
    │   └── handlers/
    │       ├── mod.rs              # EventHandler trait
    │       ├── database_sync.rs    # DB sync handler
    │       ├── health_check.rs     # Health check handler
    │       ├── lifecycle.rs        # Lifecycle handler
    │       └── monitoring.rs       # Monitoring handler
    ├── process/
    │   ├── mod.rs                  # Process manager
    │   ├── cleanup.rs              # Process termination
    │   ├── monitor.rs              # Process monitoring
    │   ├── pid_manager.rs          # PID tracking
    │   ├── spawner.rs              # Process spawning
    │   └── utils.rs                # Process utilities
    ├── registry/
    │   ├── mod.rs                  # Registry manager
    │   ├── manager.rs              # RegistryManager implementation
    │   ├── trait_impl.rs           # McpRegistry, McpToolProvider, McpDeploymentProvider impls
    │   └── validator.rs            # RegistryValidator
    ├── schema/
    │   ├── mod.rs                  # Schema service
    │   ├── loader.rs               # Schema loading
    │   └── validator.rs            # Schema validation
    ├── tool_provider/
    │   ├── mod.rs                  # McpToolProvider implementation
    │   ├── context.rs              # Tool invocation context
    │   └── conversions.rs          # Type conversions
    └── ui_renderer/
        ├── mod.rs                  # UI renderer entry point
        ├── csp.rs                  # CspPolicy builder
        └── registry.rs             # Renderer registry
```

## Database

Schemas (in `schema/`):

- `mcp_tool_executions.sql` — tool execution audit trail (`ToolUsageRepository`).
- `mcp_artifacts.sql` — persisted MCP tool output artifacts (`McpArtifactRepository`).
- `mcp_sessions.sql` — MCP session state for cross-restart resumption (`McpSessionRepository`).

## License

BSL-1.1 (Business Source License). Source-available for evaluation, testing, and non-production use. Production use requires a commercial license. Each version converts to Apache 2.0 four years after publication. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).

---

<div align="center">

**[systemprompt.io](https://systemprompt.io)** · **[Documentation](https://systemprompt.io/documentation/)** · **[Guides](https://systemprompt.io/guides)** · **[Live Demo](https://systemprompt.io/features/demo)** · **[Template](https://github.com/systempromptio/systemprompt-template)** · **[crates.io](https://crates.io/crates/systemprompt-mcp)** · **[docs.rs](https://docs.rs/systemprompt-mcp)** · **[Discord](https://discord.gg/wkAbSuPWpr)**

<sub>Domain layer · Own how your organization uses AI.</sub>

</div>
