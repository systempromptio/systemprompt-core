# systemprompt-gateway-entry

[![Crates.io](https://img.shields.io/crates/v/systemprompt-gateway-entry.svg?style=flat-square)](https://crates.io/crates/systemprompt-gateway-entry)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-gateway-entry?style=flat-square)](https://docs.rs/systemprompt-gateway-entry)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The AI gateway routes and telemetry export of systemprompt.io.

**Layer**: Entry. The `http` feature carries the routes; the crate's composition member contributes them, which the API server mounts. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
