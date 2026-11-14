# systemprompt-mcp-host

[![Crates.io](https://img.shields.io/crates/v/systemprompt-mcp-host.svg?style=flat-square)](https://crates.io/crates/systemprompt-mcp-host)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-mcp-host?style=flat-square)](https://docs.rs/systemprompt-mcp-host)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The least-privilege child hosts of the systemprompt.io API: the context a managed MCP server or a spawned agent boots on, the host that serves a YAML-declared MCP server's `toolsets:`, and the agent host.

**Layer**: App. It depends on `systemprompt-runtime` for the composition stages (`HostKernel`) and on the domain crates whose handles a host holds. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.

## Modes of an internal MCP server

| Manifest | Spawned as |
|----------|------------|
| `binary: <name>` | the extension binary `<name>` |
| no `binary:`, `toolsets: [...]` | this product binary, `systemprompt admin mcp serve --server <id>` |

Both receive the same cleared environment (profile path, database URLs, the public verification key set, the granted secrets) and the same spawn ownership marker. A hosted server's toolset ids are checked against the toolsets linked into the binary when the API starts.

## Public surface

| Item | Purpose |
|------|---------|
| `HostContext` | The child's context: `from_process_env()` / `boot(lookup)` |
| `toolsets::{serve, serve_on, build_server}` | Serve one YAML-declared MCP server |
| `submit_toolset!` | Register a toolset factory (and its id for startup validation) |
| `AgentHost` | The agent's composition over a `HostContext` |

## License

Business Source License 1.1. See [LICENSE](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE).
