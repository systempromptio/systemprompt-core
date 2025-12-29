//! CLI-specific OAuth templates
//!
//! This module contains HTML templates for OAuth callback responses.
//! The actual OAuth flow logic is in systemprompt-cloud crate.

mod templates;

pub use templates::{ERROR_HTML, SUCCESS_HTML};
