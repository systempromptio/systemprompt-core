# systemprompt-device

[![Crates.io](https://img.shields.io/crates/v/systemprompt-device.svg?style=flat-square)](https://crates.io/crates/systemprompt-device)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-device?style=flat-square)](https://docs.rs/systemprompt-device)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Device-side sign-in, keyrings, configuration and policy stores, manifest sync and acceptance for systemprompt.io.

| Module | Holds |
|---|---|
| `protocol` | The wire contract with an instance: no IO, no runtime, no HTTP client. Bodies are `systemprompt-models` types. |
| `secure` | Sealing of secrets for the current user, the key store and private files and directories, with one implementation per platform. |
| `client` | The HTTP client for one instance: discovery, the device-link sign-in, sessions, the manifest and its verification. |
| `accounts` | The accounts of this install: the instances the device knows, the sign-in flows and credential renewal. |
| `view` | What a window may see of accounts and sync; nothing in it holds a credential. |

It depends on `systemprompt-identifiers`, `systemprompt-models` and `systemprompt-traits`.

**Layer**: Infra. Device infrastructure consumed by the device domain and application crates; it depends on no server crate above the shared layer. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
