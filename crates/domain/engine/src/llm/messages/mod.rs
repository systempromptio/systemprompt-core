//! The Anthropic Messages wire format: request body shaping and stream
//! decoding.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod content;
mod decoder;
mod request;

pub(crate) use decoder::MessagesDecoder;
pub(crate) use request::body;

pub(crate) const API_VERSION: &str = "2023-06-01";
