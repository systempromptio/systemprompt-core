# systemprompt-content-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-content-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-content-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-content-entry?style=flat-square)](https://docs.rs/systemprompt-content-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The content HTTP routes and CLI commands of systemprompt.io: published content, links and engagement, with the templates and generator commands that render it.

**Layer**: Entry. The `http` feature carries the routes, declared through `systemprompt-extension` and mounted by the API server; the `cli` feature carries the commands, registered with the CLI. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
