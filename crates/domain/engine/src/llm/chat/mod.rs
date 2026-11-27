//! The `OpenAI` Chat Completions wire format: request body shaping and stream
//! decoding.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod decoder;
mod request;

pub(crate) use decoder::ChatDecoder;
pub(crate) use request::body;
