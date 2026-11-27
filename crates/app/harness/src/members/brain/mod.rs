//! The Brain, as an extension: an organisation's shared memory of revisioned
//! facts over the instance's `/api/v1/brain` routes.
//!
//! - [`client`]: [`BrainClient`], one account's brain routes (facts, their
//!   timelines and why-views, subjects, entering and governing facts, disputes,
//!   proposals), called with the account's JWT through [`sp_client::with_jwt`].
//! - [`error`]: [`BrainError`], how the instance's answers map to failures; a
//!   brain the instance did not enable is [`BrainError::Disabled`].
//! - [`service`]: [`BrainService`], the brain of the applied instance as the
//!   shell and the tools share it, with its status, its change feed and the
//!   facts injected into each session; [`BrainProbe`] reads whether an attached
//!   instance's brain is enabled.
//! - [`engine`]: [`BrainEngine`], the tools `brain_lookup`, `brain_why`,
//!   `brain_propose` and `brain_dispute`, the recall switch and the memory
//!   observer.
//! - [`extension`]: [`BrainExtension`], registered as extension `brain`.
//!
//! It fails closed: nothing reaches the brain unless the instance admits the
//! extension for this host and has confirmed its brain is enabled. Every
//! credential stays in memory and goes only to the account's own origin.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

/// [`BrainClient`] over the brain routes.
pub mod client;
/// The engine half: tools, prompt guidance and call hooks.
pub mod engine;
/// How the instance's answers map to failures.
pub mod error;
/// The instance-side half and its registration.
pub mod extension;
mod http;
mod memory;
/// The shared brain of the applied instance.
pub mod service;
/// The person's brain preferences.
pub mod settings;

pub use client::{BrainClient, FactQuery};
pub use engine::BrainEngine;
pub use error::BrainError;
pub use extension::{BRAIN_EXTENSION, BrainExtension};
pub use service::{BrainFeed, BrainProbe, BrainService, BrainStatus};
pub use settings::BrainSettings;
