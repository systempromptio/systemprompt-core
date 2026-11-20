# systemprompt-mcp-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-mcp-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-mcp-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-mcp-entry?style=flat-square)](https://docs.rs/systemprompt-mcp-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The MCP server registry route of systemprompt.io.

**Layer**: Entry. The `http` feature carries the route; `McpHttpExtension` contributes it to the composition, which the API server mounts. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Features

| Feature | Carries |
|---------|---------|
| `http` | `McpHttpExtension` (id `mcp-http`, depends on `mcp`): `GET /api/v1/mcp/registry`, the enabled MCP servers with their client endpoints, public, in the MCP-and-stream role group under the `mcp_registry` rate limit. The route reads the MCP server registry the `mcp` extension provides. |
| `cli` | no command: `plugins mcp` is served by the CLI kernel. |

The MCP executions and proxy routes under `/api/v1/mcp`, the MCP OAuth discovery documents and `admin mcp` belong to the kernel.

## Disabling

| Profile disables | Route | Boot |
|------------------|-------|------|
| `mcp-http` | not mounted; `/api/v1/mcp/registry` answers 404 | boots |
| `mcp` | — | refused: `mcp-http` depends on it |
