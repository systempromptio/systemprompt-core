//! Inventory-collected gateway request guards.
//!
//! A [`GatewayRequestGuard`] is consulted on every gateway request, right after
//! the quota precheck. Extensions register guards with
//! [`register_gateway_guard!`](macro@crate::register_gateway_guard) (mirroring
//! [`register_extension!`](crate::register_extension)), naming the extension
//! that owns each guard; the gateway runs every collected guard and denies
//! the request on the first [`GatewayDenyReason`].
//!
//! [`GatewayGuards::discover`] (over the linked registrations and the
//! profile's disabled set; [`GatewayGuards::from_registrations`] over any)
//! installs a guard only while its owning extension is linked and enabled,
//! in registration order, as every other owner-attributed registration is
//! collected. A guard whose owner is disabled in the profile is not
//! installed: the skip is logged at `warn` with the owners and the count,
//! and [`GatewayGuards::not_installed`] lists the owners so the server
//! reports them in its startup summary. A guard naming an owner that is not
//! a linked extension is refused with [`GatewayGuardError::UnknownOwner`], so
//! a mistyped owner never silently drops a guard from the request path.
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

use crate::context::ExtensionContext;

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

/// Compile-time registration of a [`GatewayRequestGuard`].
///
/// Collected via `inventory`. `extension` is the id of the owning extension.
/// The factory receives the extension context so a guard can hold the
/// services, subject attributes or token cache it needs.
#[derive(Debug, Clone, Copy)]
pub struct GatewayRequestGuardRegistration {
    pub extension: &'static str,
    pub factory: fn(&dyn ExtensionContext) -> Arc<dyn GatewayRequestGuard>,
}

inventory::collect!(GatewayRequestGuardRegistration);

#[macro_export]
macro_rules! register_gateway_guard {
    ($extension:expr, factory = $factory:expr) => {
        $crate::__inventory::submit! {
            $crate::GatewayRequestGuardRegistration {
                extension: $extension,
                factory: $factory,
            }
        }
    };
    ($extension:expr, $guard_type:ty) => {
        $crate::__inventory::submit! {
            $crate::GatewayRequestGuardRegistration {
                extension: $extension,
                factory: |_| ::std::sync::Arc::new(<$guard_type>::default())
                    as ::std::sync::Arc<dyn $crate::GatewayRequestGuard>,
            }
        }
    };
}

/// Why the gateway's guard set could not be built.
#[derive(Debug, thiserror::Error)]
pub enum GatewayGuardError {
    #[error("a gateway guard names owner '{0}', which is not a linked extension")]
    UnknownOwner(ExtensionId),
}

/// The guards one gateway owns, built once at the composition root from the
/// registered factories, with the owners of the guards it left out.
#[derive(Clone, Default)]
pub struct GatewayGuards {
    installed: Vec<(&'static str, Arc<dyn GatewayRequestGuard>)>,
    not_installed: Vec<&'static str>,
}

impl std::fmt::Debug for GatewayGuards {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("GatewayGuards")
            .field("installed", &self.installed_owners())
            .field("not_installed", &self.not_installed)
            .finish()
    }
}

impl GatewayGuards {
    pub fn registrations() -> impl Iterator<Item = &'static GatewayRequestGuardRegistration> {
        inventory::iter::<GatewayRequestGuardRegistration>.into_iter()
    }

    pub fn discover(ctx: &dyn ExtensionContext) -> Result<Self, GatewayGuardError> {
        let profile = ctx.profile();
        Self::from_registrations(ctx, Self::registrations(), &profile.extensions.disabled)
    }

    pub fn from_registrations<'a>(
        ctx: &dyn ExtensionContext,
        registrations: impl IntoIterator<Item = &'a GatewayRequestGuardRegistration>,
        disabled: &[ExtensionId],
    ) -> Result<Self, GatewayGuardError> {
        let mut guards = Self::default();
        for registration in registrations {
            if !ctx.has_extension(registration.extension) {
                return Err(GatewayGuardError::UnknownOwner(ExtensionId::new(
                    registration.extension,
                )));
            }
            if disabled.iter().any(|id| id == registration.extension) {
                guards.not_installed.push(registration.extension);
                continue;
            }
            guards
                .installed
                .push((registration.extension, (registration.factory)(ctx)));
        }
        if !guards.not_installed.is_empty() {
            tracing::warn!(
                owners = ?guards.not_installed,
                count = guards.not_installed.len(),
                reason = "extension_disabled",
                "Gateway guards not installed"
            );
        }
        Ok(guards)
    }

    #[must_use]
    pub fn installed_owners(&self) -> Vec<&'static str> {
        self.installed.iter().map(|(owner, _)| *owner).collect()
    }

    #[must_use]
    pub fn not_installed(&self) -> &[&'static str] {
        &self.not_installed
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
