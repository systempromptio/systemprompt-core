//! The extension contract of the engine.
//!
//! An extension adds tools, prompt sections, agents, commands and MCP servers,
//! and may observe governed calls, review a tool call the Governor allowed or
//! add headers to a model call. Extensions are resolved once into an
//! [`ExtensionSet`] (one per id, the highest priority shadowing the rest,
//! dependencies checked) and read through the instance's
//! [`EnabledExtensions`]: an extension the instance did not enable
//! contributes nothing unless it is required.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod contribution;
mod enabled;
mod engine;
mod error;
mod hooks;
mod metadata;
mod resolve;
mod set;

pub use contribution::{Contributed, ExtensionTool, PromptContext, PromptSection};
pub use enabled::EnabledExtensions;
pub use engine::EngineExtension;
pub use error::ResolveError;
pub use systemprompt_identifiers::engine::ExtensionId;
pub use crate::llm::ResponseHead;
pub use hooks::{CallAugmenter, GovernorObserver, Review, ToolReviewer, add_headers};
pub use metadata::ExtensionMetadata;
pub use resolve::{Resolved, admitted, resolve};
pub use set::ExtensionSet;
