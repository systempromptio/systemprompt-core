//! CLI session lifecycle: resolve, create, persist, and clear sessions.
//!
//! Re-exports the session surface used across commands: [`CliSessionContext`]
//! (an authenticated session bound to its profile), [`get_or_create_session`]
//! (the primary entry point), and the on-disk session store helpers
//! ([`load_session_store`], [`get_session_for_key`], [`clear_session`],
//! [`clear_all_sessions`]). The [`api`] submodule carries the remote-session
//! HTTP surface.

pub mod api;
mod context;
mod creation;
mod resolution;
mod store;

pub use context::CliSessionContext;
pub use resolution::get_or_create_session;
pub use store::{clear_all_sessions, clear_session, get_session_for_key, load_session_store};
