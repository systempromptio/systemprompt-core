# systemprompt-slack-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-slack-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-slack-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-slack-entry?style=flat-square)](https://docs.rs/systemprompt-slack-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The Slack inbound HTTP routes of systemprompt.io: Events API, slash commands and Block Kit interactions, verified in the handler and dispatched to governed A2A agents.

**Layer**: Entry. The `http` feature carries the routes, declared through `systemprompt-extension` and mounted by the API server; the `cli` feature carries the commands, registered with the CLI. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
