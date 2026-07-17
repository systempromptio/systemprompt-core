//! Static and prerendered content serving.
//!
//! Serves prerendered HTML, static assets, homepage, and metadata files from
//! the web dist directory, falling back to the content repository when a page
//! is known but not prerendered. Re-exports the matcher
//! ([`StaticContentMatcher`]), the serving state ([`StaticContentState`]), and
//! the session helpers ([`SessionInfo`], [`ensure_session`]).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod config;
pub mod fallback;
pub mod homepage;
pub mod session;
pub mod static_files;

pub use config::StaticContentMatcher;
pub use fallback::*;
pub use homepage::serve_homepage;
pub use session::{SessionInfo, ensure_session};
pub use static_files::{StaticContentState, serve_static_content};
