//! `systemprompt-content-entry` — the content entry surface.
//!
//! The `http` feature carries the capability's routes: published content,
//! search and link analytics under `/api/v1/content`, link generation for a
//! signed-in user under the same base path, and the short-link redirect
//! `/r/{short_code}` at the web root. [`extension::ContentHttpExtension`]
//! declares all three through `Extension::routers`, so the API server mounts
//! them without naming this crate.
//!
//! The `cli` feature carries the capability's commands, `core content` and
//! `web sitemap`, registered under the kernel's `core` and `web` groups.
//!
//! Either feature links `systemprompt-generator`, whose extension registers the
//! content and page prerender jobs.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#[cfg(feature = "cli")]
pub mod cli;
#[cfg(feature = "http")]
pub mod extension;
#[cfg(feature = "http")]
pub mod http;

#[cfg(any(feature = "http", feature = "cli"))]
use systemprompt_generator as _;
