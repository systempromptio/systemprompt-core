# systemprompt-engine

[![Crates.io](https://img.shields.io/crates/v/systemprompt-engine.svg?style=flat-square)](https://crates.io/crates/systemprompt-engine)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-engine?style=flat-square)](https://docs.rs/systemprompt-engine)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The native agent engine of systemprompt.io devices: agent loop, tools, MCP, LSP, workspace, store, configuration, prompts and model transport.

One top-level module per part, in dependency order (a module names only those before it; `just lint-module-order` holds the order):

| Module | Holds |
|---|---|
| `config` | What the engine reads from disk about how to behave; every location is injected. |
| `store` | The durable session model in one SQLite database: projects, sessions, messages, parts, todo lists. |
| `llm` | Streaming model clients for the Anthropic Messages and OpenAI Chat Completions protocols a gateway serves. |
| `tools` | The tool contract and the built-in tools. |
| `mcp` | The Model Context Protocol client: remote and stdio servers. |
| `workspace` | Per-directory services around file changes: snapshots, formatting, watching. |
| `lsp` | The language-server client. |
| `prompt` | Every text the engine shows a model, and the assembly of the system prompt. |
| `run` | The agent engine: sessions, the run loop, permissions, compaction and subagents, governed through the `Governor` it is built with. |

It depends on `systemprompt-identifiers` and `systemprompt-traits` and on no other workspace crate.

**Layer**: Domain. A device domain crate and a peer of the other device domain crates: they meet through slots and shared-layer traits, never by depending on each other. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
