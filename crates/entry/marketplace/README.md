# systemprompt-marketplace-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-marketplace-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-marketplace-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-marketplace-entry?style=flat-square)](https://docs.rs/systemprompt-marketplace-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The marketplace and managed-resource HTTP routes and CLI commands of systemprompt.io.

**Layer**: Entry. The `http` feature carries the routes and the `cli` feature the commands; the crate's composition member contributes both, which the API server and the CLI mount. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
