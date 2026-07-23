//! Inventory-collected gateway request guards.
//!
//! A [`GatewayRequestGuard`] is consulted on every gateway request, right after
//! the quota precheck. Extensions register guards with [`register_gateway_guard!`](macro@crate::register_gateway_guard)
//! (mirroring [`register_extension!`](crate::register_extension)); the gateway
//! runs every collected guard and denies the request on the first
//! [`GatewayDenyReason`]. This lets an extension enforce a policy — for example a
//! per-user credit balance — without the core needing to know about it.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

/// Why a gateway request was denied by a guard. Mapped by the gateway onto the
/// same quota-exceeded response path.
#[derive(Debug, Clone)]
pub struct GatewayDenyReason {
    pub message: String,
    pub retry_after_seconds: i32,
}

impl GatewayDenyReason {
    pub fn new(message: impl Into<String>) -> Self {
        Self {
            message: message.into(),
            retry_after_seconds: 0,
        }
    }
}

/// A policy consulted on every gateway request after the quota precheck.
#[async_trait::async_trait]
pub trait GatewayRequestGuard: Send + Sync {
    /// Return `Err` to deny the request for `user_id`.
    async fn check(&self, pool: &sqlx::PgPool, user_id: &str) -> Result<(), GatewayDenyReason>;
}

/// Compile-time registration of a [`GatewayRequestGuard`], collected via
/// `inventory`.
#[derive(Debug, Clone, Copy)]
pub struct GatewayRequestGuardRegistration {
    pub factory: fn() -> Arc<dyn GatewayRequestGuard>,
}

inventory::collect!(GatewayRequestGuardRegistration);

/// Register a [`GatewayRequestGuard`] implementation with the gateway.
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

/// Run every registered guard in turn; the first denial wins. Returns `Ok(())`
/// when no guards are registered.
pub async fn run_gateway_guards(
    pool: &sqlx::PgPool,
    user_id: &str,
) -> Result<(), GatewayDenyReason> {
    for registration in inventory::iter::<GatewayRequestGuardRegistration> {
        let guard = (registration.factory)();
        guard.check(pool, user_id).await?;
    }
    Ok(())
}
