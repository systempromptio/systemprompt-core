# systemprompt-hosts

[![Crates.io](https://img.shields.io/crates/v/systemprompt-hosts.svg?style=flat-square)](https://crates.io/crates/systemprompt-hosts)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-hosts?style=flat-square)](https://docs.rs/systemprompt-hosts)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The host contract of systemprompt.io devices, with one Cargo feature per governed product.

The crate declares no items and has no dependencies.

**Layer**: Domain. A device domain crate and a peer of the other device domain crates: they meet through slots and shared-layer traits, never by depending on each other. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
