//! `GatewayGuards::from_registrations`, the selection `discover` runs over
//! the linked registrations: a guard whose owner is enabled is installed, a
//! guard whose owner is disabled is skipped with a warn record naming the
//! owners and the count and is listed by `not_installed`, enabled owners'
//! guards keep their registration order, and a guard naming an owner that
//! is not a linked extension is refused with `GatewayGuardError::UnknownOwner`.

use std::sync::{Arc, Mutex};

use systemprompt_extension::{
    ExtensionContext, GatewayDenyReason, GatewayGuardError, GatewayGuardRequest, GatewayGuards,
    GatewayRequestGuard, GatewayRequestGuardRegistration,
};
use systemprompt_identifiers::ExtensionId;
use systemprompt_traits::DatabaseHandle;
use tracing_subscriber::layer::SubscriberExt;

use crate::stub_ctx::{GUARD_OWNER, StubCtx};

struct InertGuard;

#[async_trait::async_trait]
impl GatewayRequestGuard for InertGuard {
    async fn check(
        &self,
        _db: &dyn DatabaseHandle,
        _request: &GatewayGuardRequest<'_>,
    ) -> Result<(), GatewayDenyReason> {
        Ok(())
    }
}

fn inert(_ctx: &dyn ExtensionContext) -> Arc<dyn GatewayRequestGuard> {
    Arc::new(InertGuard)
}

const fn owned_by(extension: &'static str) -> GatewayRequestGuardRegistration {
    GatewayRequestGuardRegistration {
        extension,
        factory: inert,
    }
}

#[derive(Clone, Default)]
struct Capture(Arc<Mutex<Vec<u8>>>);

impl std::io::Write for Capture {
    fn write(&mut self, bytes: &[u8]) -> std::io::Result<usize> {
        self.0.lock().expect("capture").extend_from_slice(bytes);
        Ok(bytes.len())
    }
    fn flush(&mut self) -> std::io::Result<()> {
        Ok(())
    }
}

impl<'a> tracing_subscriber::fmt::MakeWriter<'a> for Capture {
    type Writer = Self;
    fn make_writer(&'a self) -> Self::Writer {
        self.clone()
    }
}

#[test]
fn an_enabled_owners_guard_is_installed() {
    let guards = GatewayGuards::from_registrations(&StubCtx, &[owned_by(GUARD_OWNER)], &[])
        .expect("the owner is linked");
    assert_eq!(guards.len(), 1);
}

#[test]
fn a_disabled_owners_guard_is_skipped_and_the_skip_names_the_owner() {
    let records = Capture::default();
    let subscriber = tracing_subscriber::registry().with(
        tracing_subscriber::fmt::layer()
            .json()
            .with_target(false)
            .with_writer(records.clone()),
    );
    let _subscriber = tracing::subscriber::set_default(subscriber);

    let guards = GatewayGuards::from_registrations(
        &StubCtx,
        &[owned_by(GUARD_OWNER)],
        &[ExtensionId::new(GUARD_OWNER)],
    )
    .expect("a disabled owner is not an error");
    assert!(guards.is_empty());
    assert_eq!(guards.not_installed(), &[GUARD_OWNER]);

    let logged = String::from_utf8(records.0.lock().expect("capture").clone()).expect("utf-8");
    assert!(logged.contains("Gateway guards not installed"), "{logged}");
    assert!(logged.contains("\"level\":\"WARN\""), "{logged}");
    assert!(logged.contains(GUARD_OWNER), "{logged}");
    assert!(logged.contains("\"count\":1"), "{logged}");
    assert!(logged.contains("extension_disabled"), "{logged}");
}

#[test]
fn an_unknown_owner_is_refused_naming_the_owner() {
    let refused = GatewayGuards::from_registrations(
        &StubCtx,
        &[owned_by(GUARD_OWNER), owned_by("not-a-linked-extension")],
        &[],
    );
    match refused {
        Err(GatewayGuardError::UnknownOwner(owner)) => {
            assert_eq!(owner, "not-a-linked-extension");
            assert!(
                GatewayGuardError::UnknownOwner(owner)
                    .to_string()
                    .contains("not-a-linked-extension")
            );
        },
        other => panic!("expected UnknownOwner, got {other:?}"),
    }
}

#[test]
fn a_disabled_owner_does_not_excuse_an_unknown_one() {
    let refused = GatewayGuards::from_registrations(
        &StubCtx,
        &[owned_by("not-a-linked-extension")],
        &[ExtensionId::new("not-a-linked-extension")],
    );
    assert!(matches!(refused, Err(GatewayGuardError::UnknownOwner(_))));
}

#[test]
fn enabled_guards_are_installed_in_registration_order() {
    const SECOND: &str = "stub-guard-owner-second";
    const THIRD: &str = "stub-guard-owner-third";
    let registrations = [owned_by(SECOND), owned_by(GUARD_OWNER), owned_by(THIRD)];
    let guards = GatewayGuards::from_registrations(&StubCtx, &registrations, &[])
        .expect("every owner is linked");
    assert_eq!(guards.installed_owners(), vec![SECOND, GUARD_OWNER, THIRD]);
    assert!(guards.not_installed().is_empty());

    let without_second =
        GatewayGuards::from_registrations(&StubCtx, &registrations, &[ExtensionId::new(SECOND)])
            .expect("a disabled owner is not an error");
    assert_eq!(without_second.installed_owners(), vec![GUARD_OWNER, THIRD]);
    assert_eq!(without_second.not_installed(), &[SECOND]);
}
