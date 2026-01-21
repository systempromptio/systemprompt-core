pub mod client;
pub mod clients;
pub mod core;
pub mod discovery;
pub mod health;
pub mod endpoints;
mod responses;
pub mod webauthn;
pub mod wellknown;

pub use core::{authenticated_router, public_router, router};
pub use health::*;
pub use wellknown::wellknown_routes;
