//! `systemprompt-content-entry` — the content entry surface.
//!
//! The `http` feature carries the capability's routes: published content,
//! search and link analytics under `/api/v1/content`, link generation for a
//! signed-in user under the same base path, and the short-link redirect
//! `/r/{short_code}` at the web root. The `cli` feature carries the
//! capability's commands, `core content` and `web sitemap`.
//! [`extension::ContentHttpExtension`] contributes both to the composition,
//! so neither the API server nor the CLI names this crate.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "cli")]
pub mod cli;
pub mod extension;
#[cfg(feature = "http")]
pub mod http;
