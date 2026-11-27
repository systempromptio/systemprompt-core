# systemprompt-drive

[![Crates.io](https://img.shields.io/crates/v/systemprompt-drive.svg?style=flat-square)](https://crates.io/crates/systemprompt-drive)
[![Docs.rs](https://img.shields.io/docsrs/systemprompt-drive?style=flat-square)](https://docs.rs/systemprompt-drive)
[![License: BSL-1.1](https://img.shields.io/badge/license-BSL--1.1-2b6cb0?style=flat-square)](https://github.com/systempromptio/systemprompt-core/blob/main/LICENSE)

Folder sync for systemprompt.io devices.

| Module | Holds |
|---|---|
| `client` | `DriveClient`: one account's document routes, called with the account's JWT. |
| `error` | `DriveError`: how the instance's answers map to failures a caller can act on. |
| `upload` | The part plan of a resumable upload, progress reports and cancellation. |
| `sync` | `DriveSync`: a local folder mirrored with the instance over the Drive sync feed. |
| `view` | The document rows a listing is shown as. |

The Drive's member of the device composition, with its agent tools, is in `systemprompt-harness`. It depends on `systemprompt-device` and the shared layer.

**Layer**: Domain. A device domain crate and a peer of the other device domain crates: they meet through slots and shared-layer traits, never by depending on each other. Part of the [systemprompt-core](https://github.com/systempromptio/systemprompt-core) workspace.
