//! Google Gemini `generateContent` API wire types.
//!
//! Re-exports the request and response structs matching Gemini's JSON shape.
//! The model catalogue (ids, pricing, capabilities) lives in the profile
//! `providers` registry, not here.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod request;
mod response;

pub use request::*;
pub use response::*;
