//! `WebAuthn` passkey registration flow.
//!
//! Exposes the paired [`start_register`]/[`finish_register`] ceremony that
//! enrols a new user's first credential.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod finish;
mod start;

pub use finish::finish_register;
pub use start::start_register;
