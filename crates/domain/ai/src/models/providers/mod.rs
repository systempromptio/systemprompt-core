//! Gemini image-generation wire types.
//!
//! Chat/completions wire translation lives once in the shared
//! `systemprompt_models::wire` codec; the only vendor shapes that remain here
//! are the Gemini [`gemini`] image-generation request/response, which the image
//! provider uses against the `generateContent` image models.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod gemini;
