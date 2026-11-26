# systemprompt-engine

[![Crates.io](https://img.shields.io/crates/v/systemprompt-engine.svg?style=flat-square)](https://crates.io/crates/systemprompt-engine)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-engine?style=flat-square)](https://docs.rs/systemprompt-engine)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The native agent engine of systemprompt.io devices: agent loop, tools, MCP, LSP, workspace, store, configuration, prompts and model transport.

The crate declares no items and has no dependencies.

**Layer**: Domain. A device domain crate and a peer of the other device domain crates: they meet through slots and shared-layer traits, never by depending on each other. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
