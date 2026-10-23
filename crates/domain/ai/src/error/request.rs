//! [`RequestError`] (a request the chosen provider cannot accept) and
//! [`ToolError`] (tool schemas and tool discovery).
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use thiserror::Error;

use systemprompt_identifiers::{AgentName, McpToolName, ProviderId};
use systemprompt_traits::{ServerListingFailure, ToolProviderError};

use crate::models::image_generation::{AspectRatio, ImageResolution};

#[derive(Debug, Error)]
pub enum RequestError {
    #[error("provider {provider}: prompt length {length} exceeds maximum {max}")]
    PromptTooLong {
        provider: ProviderId,
        length: usize,
        max: usize,
    },

    #[error("provider {provider} does not support resolution {}", .resolution.as_str())]
    UnsupportedResolution {
        provider: ProviderId,
        resolution: ImageResolution,
    },

    #[error("provider {provider} does not support aspect ratio {}", .aspect_ratio.as_str())]
    UnsupportedAspectRatio {
        provider: ProviderId,
        aspect_ratio: AspectRatio,
    },
}

#[derive(Debug, Error)]
pub enum ToolError {
    #[error("Tool {tool_name} missing required field: {field}")]
    MissingField {
        tool_name: McpToolName,
        field: &'static str,
    },

    #[error("Tool description cannot be empty for tool: {tool_name}")]
    EmptyDescription { tool_name: McpToolName },

    #[error("tool inventory for agent {agent} is incomplete: {}", describe_failures(.failed))]
    DiscoveryIncomplete {
        agent: AgentName,
        failed: Vec<ServerListingFailure>,
    },

    #[error(transparent)]
    Provider(ToolProviderError),
}

fn describe_failures(failed: &[ServerListingFailure]) -> String {
    failed
        .iter()
        .map(|f| format!("{}: {}", f.server, f.message))
        .collect::<Vec<_>>()
        .join("; ")
}
