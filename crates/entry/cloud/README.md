# systemprompt-cloud-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-cloud-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-cloud-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-cloud-entry?style=flat-square)](https://docs.rs/systemprompt-cloud-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The cloud CLI commands of systemprompt.io: tenant sign-in, profiles, deploy, secrets and fleet operations against the hosted control plane.

**Layer**: Entry. The `http` feature carries the routes, declared through `systemprompt-extension` and mounted by the API server; the `cli` feature carries the commands, registered with the CLI. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
