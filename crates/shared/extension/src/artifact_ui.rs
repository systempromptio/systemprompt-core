//! The artifact UI rendering contract: an artifact rendered as an MCP App
//! document for a surface outside the MCP capability.
//!
//! The `mcp` member owns the UI renderers (its built `UiSlot`) and provides
//! them as [`DynArtifactUiRenderer`], open: rendering reads only the artifact
//! the caller already holds. The `agent-http` artifact routes read it from
//! their route state; a composition without the `mcp` member provides no
//! renderer, and an artifact UI request then fails with the fixed 5xx body.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_models::a2a::Artifact;
use systemprompt_traits::BoxedSource;

/// An artifact rendered as an MCP App document.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct RenderedArtifactUi {
    pub html: String,
    pub content_security_policy: String,
}

#[derive(Debug, thiserror::Error)]
pub enum ArtifactUiError {
    #[error("no UI renderer for artifact type '{artifact_type}'")]
    Unsupported { artifact_type: String },
    #[error("the artifact UI renderer failed")]
    Render(#[source] BoxedSource),
}

/// Renders an artifact as an MCP App document.
pub trait ArtifactUiRenderer: Send + Sync {
    fn render(&self, artifact: &Artifact) -> Result<RenderedArtifactUi, ArtifactUiError>;
}

pub type DynArtifactUiRenderer = Arc<dyn ArtifactUiRenderer>;
