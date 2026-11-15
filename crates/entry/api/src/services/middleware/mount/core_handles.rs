//! Kernel handles a core declaration row is handed, and the one place they
//! are applied.
//!
//! A [`KernelHandle`] is a process handle no extension can read from the
//! shared extension state. The kernel, not the extension, decides which
//! [`CoreDeclaration`] row receives one ([`CoreDeclaration::kernel_handles`]);
//! [`hand_kernel_handles`] layers it onto a declared router as an axum
//! `Extension` only when that router's declaration matches the row in full
//! (owner id, base path, route group and protection shape). It runs after
//! `admit_extensions`, so a declaration that drifts from its row has already
//! refused boot, and a declaration from any other extension id matches no
//! row and receives nothing.
//!
//! The only handle is the analytics event broadcaster, handed to the
//! `Analytics` row (`analytics-http` at `/api/v1/analytics`), whose live
//! stream subscribes to it. Its holder can subscribe to, publish on and
//! enumerate every user's analytics stream, so it never enters the shared
//! extension state.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use axum::Extension;
use systemprompt_events::EventBroadcasters;
use systemprompt_extension::ExtensionRouter;

use super::core_declaration::{CoreDeclaration, CoreDeclarer};

/// A kernel-owned handle the closed core table hands to one row's router.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum KernelHandle {
    AnalyticsBroadcaster,
}

impl CoreDeclaration {
    fn row_of(extension: &'static str, declared: &ExtensionRouter) -> Option<Self> {
        let owner = CoreDeclarer::Extension(extension);
        Self::ALL.iter().copied().find(|entry| {
            entry.names(owner, declared.base_path, declared.group)
                && entry.shape_matches(&declared.protection)
        })
    }
}

#[must_use]
pub fn kernel_handles_for(
    extension: &'static str,
    declared: &ExtensionRouter,
) -> &'static [KernelHandle] {
    CoreDeclaration::row_of(extension, declared).map_or(&[], CoreDeclaration::kernel_handles)
}

#[must_use]
pub fn hand_kernel_handles(
    extension: &'static str,
    mut declared: ExtensionRouter,
    broadcasters: &EventBroadcasters,
) -> ExtensionRouter {
    for handle in kernel_handles_for(extension, &declared) {
        declared.router = match handle {
            KernelHandle::AnalyticsBroadcaster => declared
                .router
                .layer(Extension(Arc::clone(&broadcasters.analytics))),
        };
    }
    declared
}
