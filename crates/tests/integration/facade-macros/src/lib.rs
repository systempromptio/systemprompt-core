//! Registration macros expanded in a crate whose only core dependency is the
//! `systemprompt` facade: a macro that names a core crate by an absolute path
//! instead of `$crate` fails to compile here.

#![allow(clippy::all, clippy::pedantic, clippy::nursery, clippy::cargo)]

use systemprompt::extension::{GatewayDenyReason, GatewayGuardRequest, GatewayRequestGuard};
use systemprompt::gateway::{
    CanonicalRequest, CanonicalResponse, Finding, SafetyScanner, ScanError, ScannerSettings,
};
use systemprompt::traits::DatabaseHandle;

const SCANNER: &str = "facade_probe_scanner";

#[derive(Default)]
struct ProbeScanner;

#[async_trait::async_trait]
impl SafetyScanner for ProbeScanner {
    fn name(&self) -> &'static str {
        SCANNER
    }

    async fn scan_request(&self, _req: &CanonicalRequest) -> Result<Vec<Finding>, ScanError> {
        Ok(Vec::new())
    }

    async fn scan_response_final(
        &self,
        _response: &CanonicalResponse,
    ) -> Result<Vec<Finding>, ScanError> {
        Ok(Vec::new())
    }
}

systemprompt::gateway::register_safety_scanner!(
    |_: &dyn systemprompt::extension::ExtensionContext, _: &ScannerSettings| ProbeScanner,
    name = SCANNER
);

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

systemprompt::extension::register_gateway_guard!(ProbeGuard);

#[test]
fn the_scanner_registered_through_the_facade_is_collected() {
    let names: Vec<&str> = systemprompt::gateway::__inventory::iter::<
        systemprompt::gateway::SafetyScannerRegistration,
    >
        .into_iter()
        .map(|registration| registration.name)
        .collect();
    assert!(names.contains(&SCANNER), "{names:?}");
}

#[test]
fn the_guard_registered_through_the_facade_is_collected() {
    let registered = systemprompt::extension::__inventory::iter::<
        systemprompt::extension::GatewayRequestGuardRegistration,
    >
        .into_iter()
        .count();
    assert!(registered >= 1);
}
