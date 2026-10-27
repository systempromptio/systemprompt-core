//! Document handling for the shared document store.
//!
//! [`codecs`] holds the core [`DocumentCodec`] implementations for the text
//! formats and the [`CodecRegistry`] a composition root builds once to pick
//! the codec for each format.
//!
//! [`templates`] holds the [`TemplateRegistry`] that merges template
//! providers with the services catalog, and the core blank templates.
//!
//! [`TemplateRegistry`]: templates::TemplateRegistry
//! [`DocumentCodec`]: systemprompt_provider_contracts::documents::DocumentCodec
//! [`CodecRegistry`]: codecs::CodecRegistry
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

pub mod codecs;
pub mod templates;
