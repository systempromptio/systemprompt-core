//! Protocol-surface route mounting.
//!
//! Mounts the kernel's agent (the A2A proxy and the webhook broadcasts), MCP,
//! stream, and content/admin route groups onto the server router. Each group is
//! an [`KernelRoutes`] declaration — base path, `AuthzPolicy`, context kind,
//! rate limit, body limit — mounted through the one declaration mount in
//! `services::middleware::mount`. [`MountCtx`] carries the enforcement
//! primitives every route mount in this module tree shares.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use axum::Router;
use systemprompt_extension::{AuthzPolicy, ContextKind, RouteGroup, RouteProtection};
use systemprompt_runtime::AppContext;

use crate::services::middleware::{KernelRoutes, MountError, MountLayers, mount, protect};

mod agent;
mod content;
mod mcp;
mod stream;

pub use agent::WEBHOOK_RATE;
pub(super) use agent::{agent_routes, mount_agent, mount_agent_cards};
pub(super) use content::mount_content_and_misc;
pub(super) use mcp::mount_mcp;
pub(super) use stream::mount_stream;

pub(super) struct MountCtx<'a> {
    pub ctx: &'a AppContext,
    pub layers: &'a MountLayers,
    pub serves: &'a dyn Fn(RouteGroup) -> bool,
    pub composed: Composed<'a>,
}

/// The admitted composition and the kernel its route sets resolve against.
#[derive(Debug, Clone, Copy)]
pub struct Composed<'a> {
    pub admitted: &'a systemprompt_extension::Admitted,
    pub kernel: &'a systemprompt_runtime::Kernel,
}

impl MountCtx<'_> {
    pub(super) fn mount(&self, tree: Router, declared: KernelRoutes) -> Result<Router, MountError> {
        mount(tree, declared, self.layers, self.serves)
    }

    pub(super) fn protect(
        &self,
        router: Router,
        protection: &RouteProtection,
        base_path: &'static str,
    ) -> Result<Router, MountError> {
        protect(router, protection, self.layers, base_path)
    }
}

pub(super) const fn declare(
    router: Router,
    base_path: &'static str,
    policy: AuthzPolicy,
    context: ContextKind,
    group: RouteGroup,
) -> KernelRoutes {
    KernelRoutes::new(router, base_path, policy, context).in_group(group)
}
