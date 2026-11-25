//! The artifact UI rendering contract: an artifact rendered as an MCP App
//! document for a surface outside the MCP capability.
//!
//! The `mcp` member owns the UI renderers (its built `UiSlot`) and provides
//! them as [`DynArtifactUiRenderer`], open: rendering reads only the artifact
//! the caller already holds. The `agent-http` artifact routes read it from
//! their route state; a composition without the `mcp` member provides no
//! renderer, and an artifact UI request then fails with the fixed 5xx body.
//!
//! Every kernel is seeded with the [`HostRole`] of the process it serves. A
//! contributed renderer names where it renders ([`RenderOn`]); the `UiSlot`
//! build runs only the renderer factories whose [`RenderOn`] covers the
//! kernel's role, so a process that does not render a member's artifacts
//! neither builds nor applies that member's renderer.
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

/// The process a kernel serves: the API (and the CLI that boots its context),
/// a hosted MCP server child, or an agent child.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HostRole {
    Api,
    McpServer,
    Agent,
}

/// Where a contributed renderer renders.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum RenderOn {
    Api,
    McpServers,
    ApiAndMcpServers,
}

impl RenderOn {
    #[must_use]
    pub const fn covers(self, role: HostRole) -> bool {
        matches!(
            (self, role),
            (Self::Api | Self::ApiAndMcpServers, HostRole::Api)
                | (
                    Self::McpServers | Self::ApiAndMcpServers,
                    HostRole::McpServer
                )
        )
    }
}
