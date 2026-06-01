//! Gemini image-generation wire types.
//!
//! Chat/completions wire translation lives once in the shared
//! `systemprompt_models::wire` codec; the only vendor shapes that remain here
//! are the Gemini [`gemini`] image-generation request/response, which the image
//! provider uses against the `generateContent` image models.

pub mod gemini;
