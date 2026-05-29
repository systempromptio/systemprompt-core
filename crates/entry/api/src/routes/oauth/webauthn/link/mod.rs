//! WebAuthn passkey-linking flow for an already-authenticated user.
//!
//! Serves the link page ([`link_passkey_page`]) and the
//! [`start_link`]/[`finish_link`] ceremony that attaches a new credential to an
//! existing account.

mod finish;
mod page;
mod start;

pub use finish::finish_link;
pub use page::link_passkey_page;
pub use start::start_link;
