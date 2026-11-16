//! Gateway request guards contributed by composition members.
//!
//! A [`GatewayRequestGuard`] is consulted on every gateway request, right after
//! the quota precheck. A member contributes one with
//! `Contributions::gateway_guard(factory)`; the runtime builds every admitted
//! member's guards, core members first and then in composition order, into
//! one [`GatewayGuards`] list, and the gateway runs every guard and denies the
//! request on the first [`GatewayDenyReason`]. A disabled member's guard is
//! dropped at admission and its factory never runs.
//!
//! This lets an extension enforce a policy — for example a per-user credit
//! balance — without the core needing to know about it.
//!
//! [`GatewayRequestGuard`] is held as `Arc<dyn GatewayRequestGuard>` in the
//! [`GatewayGuards`] list, so it uses `#[async_trait]`;
//! native `async fn` in traits is not `dyn`-compatible. A guard receives the
//! database as `&dyn DatabaseHandle` and downcasts through
//! `DatabaseHandle::as_any` to the concrete handle it was compiled against;
//! the shared layer never names a pool type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_identifiers::{ApiKeyId, ExtensionId, ModelId, ProviderId, RouteId, UserId};
use systemprompt_models::attribution::RequestAttribution;
use systemprompt_traits::DatabaseHandle;


/// The resolved gateway request a guard is asked to admit or deny.
#[derive(Debug, Clone)]
pub struct GatewayGuardRequest<'a> {
    pub user_id: &'a UserId,
    pub model: &'a ModelId,
    pub route_id: Option<&'a RouteId>,
    pub provider: &'a ProviderId,
    pub streaming: bool,
    pub attribution: &'a RequestAttribution,
    pub api_key_id: Option<&'a ApiKeyId>,
}

/// How a guard denial maps onto the HTTP response.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq)]
#[non_exhaustive]
pub enum GatewayDenyKind {
    #[default]
    Quota,
    Forbidden,
    Unavailable,
}

/// Why a gateway request was denied by a guard.
#[derive(Debug, Clone)]
pub struct GatewayDenyReason {
    pub message: String,
    pub retry_after_seconds: i32,
    pub kind: GatewayDenyKind,
}

impl GatewayDenyReason {
    pub fn unavailable(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retry_after_seconds: 5,
            kind: GatewayDenyKind::Unavailable,
        }
    }

    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retry_after_seconds: 0,
            kind: GatewayDenyKind::Quota,
        }
    }

    pub fn forbidden(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retry_after_seconds: 0,
            kind: GatewayDenyKind::Forbidden,
        }
    }
}

/// A policy consulted on every gateway request after the quota precheck;
/// held as `Arc<dyn GatewayRequestGuard>`, hence `#[async_trait]`.
#[async_trait::async_trait]
pub trait GatewayRequestGuard: Send + Sync {
    async fn check(
        &self,
        db: &dyn DatabaseHandle,
        request: &GatewayGuardRequest<'_>,
    ) -> Result<(), GatewayDenyReason>;
}

/// The guards one gateway owns, built once by the runtime from every admitted
/// member's guard factories.
#[derive(Clone, Default)]
pub struct GatewayGuards {
    installed: Vec<(ExtensionId, Arc<dyn GatewayRequestGuard>)>,
}

impl std::fmt::Debug for GatewayGuards {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayGuards")
            .field("installed", &self.installed_owners())
            .finish()
    }
}

impl GatewayGuards {
    #[must_use]
    pub const fn new(installed: Vec<(ExtensionId, Arc<dyn GatewayRequestGuard>)>) -> Self {
        Self { installed }
    }

    #[must_use]
    pub fn installed_owners(&self) -> Vec<&ExtensionId> {
        self.installed.iter().map(|(owner, _)| owner).collect()
    }

    #[must_use]
    pub fn len(&self) -> usize {
        self.installed.len()
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.installed.is_empty()
    }

    pub async fn run(
        &self,
        db: &dyn DatabaseHandle,
        request: &GatewayGuardRequest<'_>,
    ) -> Result<(), GatewayDenyReason> {
        for (_, guard) in &self.installed {
            guard.check(db, request).await?;
        }
        Ok(())
    }
}
