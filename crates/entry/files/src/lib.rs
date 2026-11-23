//! `systemprompt-files-entry` — the files entry surface.
//!
//! The `cli` feature carries the capability's commands, `core files`
//! (with its `core files ai` sub-group). The `http` feature carries the
//! document store's handlers ([`http`]): documents, folders, uploads, the
//! public-link open and issues, contributed as a builder the API mount
//! layers. `/api/v1/files` stays reserved to the kernel, and stored files
//! are served by the kernel's `/files/*` static fallback.
//! [`extension::FilesHttpExtension`] contributes both to the composition, so
//! neither the CLI nor the API names this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "cli")]
pub mod cli;
pub mod extension;
#[cfg(feature = "http")]
pub mod http;
