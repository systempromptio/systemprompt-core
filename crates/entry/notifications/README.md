# systemprompt-notifications-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-notifications-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-notifications-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-notifications-entry?style=flat-square)](https://docs.rs/systemprompt-notifications-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The notification HTTP routes of systemprompt.io: streams, rules, subscriptions, the feed and escalation.

**Layer**: Entry. The `http` feature carries the routes, declared through `systemprompt-extension` and mounted by the API server; the `cli` feature carries the commands, registered with the CLI. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
