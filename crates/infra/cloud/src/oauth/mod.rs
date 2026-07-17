//! Browser-based OAuth login flow for the cloud CLI.
//!
//! Re-exports the flow entry point [`run_oauth_flow`] and its
//! [`OAuthTemplates`] for the local callback page.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod client;

pub use client::{OAuthTemplates, run_oauth_flow};
