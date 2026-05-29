//! OpenID Connect dynamic client-configuration endpoint (RFC 7592).
//!
//! Exposes the read, update, and delete operations a registered client uses to
//! manage its own configuration via its registration access token.

pub(crate) mod delete;
pub(crate) mod get;
mod update;
pub mod validation;

pub use delete::delete_client_configuration;
pub use get::get_client_configuration;
pub use update::update_client_configuration;
