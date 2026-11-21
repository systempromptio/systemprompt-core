# systemprompt-mcp-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-mcp-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-mcp-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-mcp-entry?style=flat-square)](https://docs.rs/systemprompt-mcp-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The MCP server registry route and the `plugins mcp` commands of systemprompt.io.

**Layer**: Entry. The `http` feature carries the route and the `cli` feature the commands; `McpHttpExtension` contributes both to the composition, which the API server mounts and the CLI dispatches. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Features

| Feature | Carries |
|---------|---------|
| `http` | `McpHttpExtension` (id `mcp-http`, depends on `mcp`): `GET /api/v1/mcp/registry`, the enabled MCP servers with their client endpoints, public, in the MCP-and-stream role group under the `mcp_registry` rate limit. The route reads the MCP server registry the `mcp` extension provides. |
| `cli` | `McpHttpExtension` contributes `plugins mcp` (`list`, `list-packages`, `status`, `validate`, `logs`, `tools`, `call`) under the kernel's `plugins` group with the `caller-session` grant. The commands read the services configuration, the registry and the read-only `McpServiceObserver` the `mcp` extension provides, the application paths and the database handle. `tools` and `call` connect only to a server recorded as running, presenting the caller's own session token; no command starts, stops or signals a server. |

The MCP executions and proxy routes under `/api/v1/mcp`, the MCP OAuth discovery documents, `admin mcp`, `build mcp` and `infra services` belong to the kernel.

## Disabling

| Profile disables | Route | `plugins mcp` | Boot |
|------------------|-------|---------------|------|
| `mcp-http` | not mounted; `/api/v1/mcp/registry` answers 404 | refused at dispatch | boots |
| `mcp` | — | — | refused: `mcp-http` depends on it |
