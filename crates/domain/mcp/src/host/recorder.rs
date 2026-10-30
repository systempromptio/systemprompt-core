//! The access-audit seam of the generic toolset host.
//!
//! Core decides who may list and call a tool; where the outcome is written is
//! the installation's choice. An extension implements [`McpAccessRecorder`]
//! and registers a factory with `register_mcp_access_recorder!`; the host
//! builds every registered recorder once at start and reports each outcome to
//! all of them. A recorder is best-effort by contract: it returns nothing, so
//! an audit write that fails can never fail the call it describes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt::Debug;
use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_database::DbPool;
use systemprompt_identifiers::{McpServerId, UserId};

/// What happened to one request at the host's gate: the credential was
/// accepted (`Authenticated`), the tool ran (`Used`), authentication or the
/// governance chain refused it (`Rejected`), or the caller is authenticated
/// but the tool's declared access excludes it (`Forbidden`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum McpAccessOutcome<'a> {
    Authenticated,
    Used,
    Rejected { reason: &'a str },
    Forbidden { reason: &'a str },
}

impl McpAccessOutcome<'_> {
    #[must_use]
    pub const fn action(&self) -> &'static str {
        match self {
            Self::Authenticated => "authenticated",
            Self::Used => "used",
            Self::Rejected { .. } => "rejected",
            Self::Forbidden { .. } => "forbidden",
        }
    }
}

/// One audited outcome. `user_id` is absent when no caller was identified.
#[derive(Debug, Clone, Copy)]
pub struct McpAccessEvent<'a> {
    pub server: &'a McpServerId,
    pub tool: &'a str,
    pub user_id: Option<&'a UserId>,
    pub outcome: McpAccessOutcome<'a>,
}

/// Writes host access outcomes somewhere durable.
///
/// `#[async_trait]` because recorders are held as `Arc<dyn …>`.
#[async_trait]
pub trait McpAccessRecorder: Send + Sync + Debug {
    async fn record(&self, event: McpAccessEvent<'_>);
}

pub type SharedMcpAccessRecorder = Arc<dyn McpAccessRecorder>;

/// One inventory submission per
/// [`register_mcp_access_recorder!`](crate::register_mcp_access_recorder) call.
/// The factory runs once when a host is built and must not block.
#[derive(Debug, Clone, Copy)]
pub struct McpAccessRecorderRegistration {
    pub factory: fn(&DbPool) -> SharedMcpAccessRecorder,
}

inventory::collect!(McpAccessRecorderRegistration);

#[must_use]
pub fn discover_mcp_access_recorders(pool: &DbPool) -> Vec<SharedMcpAccessRecorder> {
    inventory::iter::<McpAccessRecorderRegistration>()
        .map(|registration| (registration.factory)(pool))
        .collect()
}

#[macro_export]
macro_rules! register_mcp_access_recorder {
    ($factory:expr) => {
        $crate::__inventory::submit! {
            $crate::host::McpAccessRecorderRegistration {
                factory: $factory,
            }
        }
    };
}
