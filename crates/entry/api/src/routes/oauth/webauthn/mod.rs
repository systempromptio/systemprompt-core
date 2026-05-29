//! WebAuthn passkey ceremonies.
//!
//! Groups the three credential flows: [`authenticate`] (login), [`register`]
//! (first-time enrolment), and [`link`] (adding a passkey to an existing
//! account). Each exposes paired start/finish handlers.

pub mod authenticate;
pub mod link;
pub mod register;

pub use authenticate::{finish_auth, start_auth};
pub use link::{finish_link, start_link};
pub use register::{finish_register, start_register};
