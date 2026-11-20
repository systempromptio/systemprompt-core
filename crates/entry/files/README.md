# systemprompt-files-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-files-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-files-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-files-entry?style=flat-square)](https://docs.rs/systemprompt-files-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The file and upload HTTP routes and CLI commands of systemprompt.io.

**Layer**: Entry. The `http` feature carries the routes and the `cli` feature the commands; the crate's composition member contributes both, which the API server and the CLI mount. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
