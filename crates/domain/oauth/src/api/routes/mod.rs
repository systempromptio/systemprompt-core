pub mod client;
pub mod clients;
pub mod core;
pub mod discovery;
pub mod health;
pub mod oauth;
pub mod webauthn;

pub use core::{authenticated_router, public_router, router};
pub use health::*;
