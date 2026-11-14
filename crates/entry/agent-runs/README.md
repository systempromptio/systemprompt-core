# systemprompt-agent-runs-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-agent-runs-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-agent-runs-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-agent-runs-entry?style=flat-square)](https://docs.rs/systemprompt-agent-runs-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The agent-run HTTP routes and CLI commands of systemprompt.io: the durable run queue, its streams and the run command.

**Layer**: Entry. The `http` feature carries the routes, declared through `systemprompt-extension` and mounted by the API server; the `cli` feature carries the commands, registered with the CLI. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
