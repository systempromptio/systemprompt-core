# systemprompt-analytics-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-analytics-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-analytics-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-analytics-entry?style=flat-square)](https://docs.rs/systemprompt-analytics-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The analytics HTTP routes and CLI reports of systemprompt.io: event ingestion, live streams, and the traffic, session, cost and content reports.

**Layer**: Entry. The `http` feature carries the routes, declared through `systemprompt-extension` and mounted by the API server; the `cli` feature carries the commands, registered with the CLI. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
