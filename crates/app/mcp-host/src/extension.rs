//! [`McpHostExtension`]: the toolset host's member. It opens the
//! [`ToolsetSlot`] with core's `issues`, `documents` and `brain` toolsets, so
//! the built
//! [`Toolsets`](crate::contract::Toolsets) are present, and the hosted
//! toolset ids are checked, whenever this member is admitted.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_extension::prelude::*;

use crate::contract::ToolsetSlot;
use crate::toolsets::core_toolsets;

pub(crate) const MCP_HOST_ID: &str = "mcp_host";

#[derive(Debug, Clone, Copy, Default)]
pub struct McpHostExtension;

impl Extension for McpHostExtension {
    fn metadata(&self) -> ExtensionMetadata {
        ExtensionMetadata {
            id: MCP_HOST_ID,
            name: "MCP host",
            version: env!("CARGO_PKG_VERSION"),
        }
    }

    fn contribute(&self, c: &mut Contributions<'_>) -> Result<(), ContributionError> {
        core_toolsets::contribute(c.slot::<ToolsetSlot>());
        Ok(())
    }
}
