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
[![codecov](https://img.shields.io/codecov/c/github/systempromptio/systemprompt-core/main?style=flat-square&logo=codecov)](https://codecov.io/gh/systempromptio/systemprompt-core)

Every MCP tool call through one audited path. Native Model Context Protocol orchestration with per-server OAuth2, RBAC middleware, and tool-call governance, so no tool runs without passing the same checks and landing in the same audit trail.

**Layer**: Domain — business-logic modules that implement systemprompt.io features. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Overview

**Capabilities** · [MCP Governance](https://systemprompt.io/features/mcp-governance)

Native MCP server lifecycle, orchestration, and governance. Manages MCP process spawning, port allocation, proxy routing, RBAC middleware, schema validation, tool execution, artifact persistence, and UI rendering across systemprompt.io.

## Usage

```toml
[dependencies]
systemprompt-mcp = "0.21"
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

## Module Layout

| Module | Purpose |
|--------|---------|
| `middleware/` | Request middleware: `rbac/` (JWT and proxy-verified identity) and `session_handler/` (`DatabaseSessionHandler` implementing the rmcp `SessionManager` trait against the database). |
| `orchestration/` | Batch tool loading and service-state queries. |
| `repository/` | Compile-time-verified persistence for MCP sessions, artifacts, and tool-usage stats. |
| `services/client/` | MCP client with context-propagating HTTP transport and connection validation. |
| `services/lifecycle/` | Server startup, restart, graceful shutdown, and health. |
| `services/network/` | Port allocation, HTTP proxy, and router/CORS routing. |
| `services/orchestrator/` | `McpOrchestrator`: daemon, event bus, reconciliation, schema sync, and event handlers. |
| `services/process/` | Subprocess spawning, PID tracking, monitoring, and cleanup. |
| `services/registry/` | `RegistryManager` and the `McpRegistry`/`McpToolProvider`/`McpDeploymentProvider` trait impls. |
| `services/schema/` | Schema loading and validation. |
| `services/tool_provider/` | `McpToolProvider` implementation and tool-invocation context. |
| `services/ui_renderer/` | MCP Apps UI rendering with a CSP policy builder. |
| `models/` · `jobs/` · `cli/` | Execution/validation types, the stale-session cleanup job, and CLI command handlers. |

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
