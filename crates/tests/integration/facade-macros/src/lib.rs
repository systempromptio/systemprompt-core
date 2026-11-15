//! Registration macros expanded in a crate whose only core dependency is the
//! `systemprompt` facade: a macro that names a core crate by an absolute path
//! instead of `$crate` fails to compile here.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt::extension::{GatewayDenyReason, GatewayGuardRequest, GatewayRequestGuard};
use systemprompt::traits::DatabaseHandle;

#[derive(Default)]
struct ProbeGuard;

#[async_trait::async_trait]
impl GatewayRequestGuard for ProbeGuard {
    async fn check(
        &self,
        _db: &dyn DatabaseHandle,
        _request: &GatewayGuardRequest<'_>,
    ) -> Result<(), GatewayDenyReason> {
        Ok(())
    }
}

systemprompt::extension::register_gateway_guard!("gateway", ProbeGuard);

#[test]
fn the_guard_registered_through_the_facade_is_collected() {
    let registered = systemprompt::extension::__inventory::iter::<
        systemprompt::extension::GatewayRequestGuardRegistration,
    >
        .into_iter()
        .count();
    assert!(registered >= 1);
}
