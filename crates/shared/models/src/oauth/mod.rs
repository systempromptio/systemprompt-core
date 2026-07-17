//! OAuth client and server configuration shapes.
//!
//! [`OAuthClientConfig`] holds credentials for an outbound OAuth client;
//! [`OAuthServerConfig`] describes this service acting as an OAuth
//! authorization server.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod client;
pub mod server;

pub use client::OAuthClientConfig;
pub use server::OAuthServerConfig;
