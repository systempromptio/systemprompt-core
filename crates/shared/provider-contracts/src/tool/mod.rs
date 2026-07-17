//! Tool-provider contracts: definitions, call requests/results, and the
//! [`ToolProvider`] trait that the agent runtime uses to discover and
//! invoke tools across MCP servers and other backends.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

mod call;
mod content;
mod context;
mod definition;
mod error;
mod provider;

pub use call::{ToolCallRequest, ToolCallResult};
pub use content::ToolContent;
pub use context::ToolContext;
pub use definition::ToolDefinition;
pub use error::{ToolProviderError, ToolProviderResult};
pub use provider::ToolProvider;
