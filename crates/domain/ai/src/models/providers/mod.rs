//! Wire-format request and response types for each upstream AI provider.
//!
//! One submodule per provider ([`anthropic`], [`gemini`], [`openai`]), each
//! mirroring that vendor's JSON API. Model catalogues (ids, pricing,
//! capabilities) live in the profile `providers` registry, not here.

pub mod anthropic;
pub mod gemini;
pub mod openai;
