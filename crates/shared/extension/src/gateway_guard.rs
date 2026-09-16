//! Inventory-collected gateway request guards.
//!
//! A [`GatewayRequestGuard`] is consulted on every gateway request, right after
//! the quota precheck. Extensions register guards with
//! [`register_gateway_guard!`](macro@crate::register_gateway_guard) (mirroring
//! [`register_extension!`](crate::register_extension)); the gateway runs every
//! collected guard and denies the request on the first [`GatewayDenyReason`].
//! This lets an extension enforce a policy — for example a per-user credit
//! balance — without the core needing to know about it.
//!
//! [`GatewayRequestGuard`] is held as `Arc<dyn GatewayRequestGuard>` in the
//! inventory-built [`gateway_guards`] list, so it uses `#[async_trait]`;
//! native `async fn` in traits is not `dyn`-compatible. A guard receives the
//! database as `&dyn DatabaseHandle` and downcasts through
//! `DatabaseHandle::as_any` to the concrete handle it was compiled against;
//! the shared layer never names a pool type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::{Arc, LazyLock};

use systemprompt_identifiers::{ModelId, ProviderId, RouteId, UserId};
use systemprompt_traits::DatabaseHandle;

/// The resolved gateway request a guard is asked to admit or deny.
#[derive(Debug, Clone)]
pub struct GatewayGuardRequest<'a> {
    pub user_id: &'a UserId,
    pub model: &'a ModelId,
    pub route_id: Option<&'a RouteId>,
    pub provider: &'a ProviderId,
    pub streaming: bool,
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

/// Compile-time registration of a [`GatewayRequestGuard`], collected via
/// `inventory`.
#[derive(Debug, Clone, Copy)]
pub struct GatewayRequestGuardRegistration {
    pub factory: fn() -> Arc<dyn GatewayRequestGuard>,
}

inventory::collect!(GatewayRequestGuardRegistration);

#[macro_export]
macro_rules! register_gateway_guard {
    ($guard_type:ty) => {
        ::inventory::submit! {
            $crate::GatewayRequestGuardRegistration {
                factory: || ::std::sync::Arc::new(<$guard_type>::default())
                    as ::std::sync::Arc<dyn $crate::GatewayRequestGuard>,
            }
        }
    };
    ($guard_expr:expr) => {
        ::inventory::submit! {
            $crate::GatewayRequestGuardRegistration {
                factory: || ::std::sync::Arc::new($guard_expr)
                    as ::std::sync::Arc<dyn $crate::GatewayRequestGuard>,
            }
        }
    };
}

static GATEWAY_GUARDS: LazyLock<Vec<Arc<dyn GatewayRequestGuard>>> = LazyLock::new(|| {
    inventory::iter::<GatewayRequestGuardRegistration>
        .into_iter()
        .map(|registration| (registration.factory)())
        .collect()
});

#[must_use]
pub fn gateway_guards() -> &'static [Arc<dyn GatewayRequestGuard>] {
    &GATEWAY_GUARDS
}

pub async fn run_gateway_guards(
    db: &dyn DatabaseHandle,
    request: &GatewayGuardRequest<'_>,
) -> Result<(), GatewayDenyReason> {
    for guard in gateway_guards() {
        guard.check(db, request).await?;
    }
    Ok(())
}
