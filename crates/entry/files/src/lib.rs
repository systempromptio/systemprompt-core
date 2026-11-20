//! `systemprompt-files-entry` — the files entry surface.
//!
//! The `cli` feature carries the capability's commands, `core files`
//! (with its `core files ai` sub-group). The `http` feature carries no
//! routes: the files capability has no API of its own, `/api/v1/files` stays
//! reserved to the kernel, and stored files are served by the kernel's
//! `/files/*` static fallback. [`extension::FilesHttpExtension`] contributes
//! the commands to the composition, so the CLI does not name this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "cli")]
pub mod cli;
pub mod extension;
