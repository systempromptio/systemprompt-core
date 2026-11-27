# systemprompt-harness

[![Crates.io](https://img.shields.io/crates/v/systemprompt-harness.svg?style=flat-square)](https://crates.io/crates/systemprompt-harness)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-harness?style=flat-square)](https://docs.rs/systemprompt-harness)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

The device composition of systemprompt.io: the member contract and sink, the governor, projection, update and commands.

One top-level module per part, in dependency order (a module names only those before it; `just lint-module-order` holds the order):

| Module | Holds |
|---|---|
| `projection` | A verified manifest projected into the device's configuration directory and into the in-memory snapshot the rest reads. |
| `extension` | The instance-side extension contract and the registration type a composition lists its extensions with. |
| `governance` | The engine's `Governor` for a signed-in instance: verdicts, hooks and telemetry. |
| `members` | The Drive, Brain, notifications and Slack extensions, and `REGISTERED`, the list that names them and the governance extension. |
| `sync` | Live sync of the active account: polling, acceptance and apply of its manifest. |

It depends on `systemprompt-device`, `systemprompt-engine`, `systemprompt-drive` and the shared layer.

**Layer**: App. The device application crate: it composes the device infrastructure and domain crates and depends on no server crate above the shared layer. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
