//! HTTP route modules for the API server.
//!
//! Each submodule owns one functional area (admin, documents, gateway, oauth,
//! proxy, stream, sync, users, webhook, wellknown) and exposes
//! a `Router` builder. The
//! re-exports here surface the router constructors the API assembler composes
//! into the top-level service.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod admin;
#[cfg(feature = "brain")]
pub mod brain;
pub mod documents;
pub mod gateway;
pub mod internal;
pub mod issues;
#[cfg(feature = "bridge")]
pub mod managed;
pub(crate) mod managed_state;
pub mod oauth;
pub mod proxy;
pub mod sso;
pub mod stream;
pub mod sync;
pub mod uploads;
pub(crate) mod user_content;
pub mod users;
pub mod webhook;
pub mod wellknown;

pub use oauth::{
    authenticated_router as oauth_authenticated_router, public_router as oauth_public_router,
    revocation_router as oauth_revocation_router,
};
pub use stream::stream_router;
pub use sync::router as sync_router;
pub use wellknown::wellknown_router;
