//! OAuth client and server configuration shapes.
//!
//! [`OAuthClientConfig`] holds credentials for an outbound OAuth client;
//! [`OAuthServerConfig`] describes this service acting as an OAuth
//! authorization server.

pub mod client;
pub mod server;

pub use client::OAuthClientConfig;
pub use server::OAuthServerConfig;
