//! The toolset factory inventory.
//!
//! A crate that ships an [`McpToolset`] registers how to build it from the
//! server's [`HostContext`] with [`submit_toolset!`](crate::submit_toolset); a
//! deployment hosts it by listing the factory's `id` under `toolsets:`. The
//! macro also submits the id as a
//! [`RegisteredToolsetId`](systemprompt_runtime::RegisteredToolsetId), which
//! API startup validation checks hosted servers against.
//! `Ok(None)` from a factory means the toolset is unavailable in this
//! installation and is skipped with a warning; an `Err` fails start.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use systemprompt_identifiers::McpServerId;
use systemprompt_mcp::McpToolset;

use crate::HostContext;

pub type ToolsetBuildError = Box<dyn std::error::Error + Send + Sync + 'static>;
pub type ToolsetBuildResult = Result<Option<Box<dyn McpToolset>>, ToolsetBuildError>;

/// One registered toolset: its YAML id and how to build it for a server.
#[derive(Debug, Clone, Copy)]
pub struct ToolsetFactory {
    pub id: &'static str,
    pub build: fn(&HostContext, &McpServerId) -> ToolsetBuildResult,
}

systemprompt_mcp::__inventory::collect!(ToolsetFactory);

pub fn toolset_factories() -> impl Iterator<Item = &'static ToolsetFactory> {
    systemprompt_mcp::__inventory::iter::<ToolsetFactory>.into_iter()
}

#[must_use]
pub fn find_toolset_factory(id: &str) -> Option<&'static ToolsetFactory> {
    toolset_factories().find(|factory| factory.id == id)
}

pub use systemprompt_runtime::registered_toolset_ids;

#[macro_export]
macro_rules! submit_toolset {
    ($id:expr, $build:expr $(,)?) => {
        $crate::__toolset_inventory::submit! {
            $crate::toolsets::ToolsetFactory {
                id: $id,
                build: $build,
            }
        }
        $crate::__toolset_inventory::submit! {
            $crate::__RegisteredToolsetId { id: $id }
        }
    };
}
