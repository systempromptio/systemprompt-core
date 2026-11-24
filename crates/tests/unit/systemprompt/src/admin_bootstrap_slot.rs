//! The pre-kernel admin-bootstrap constructor is read only from a
//! core-standing `users` member: an install's constructor is never called,
//! `users` is a required member, and an admitted set without it has no admin
//! bootstrap at all.

use std::sync::Arc;

use systemprompt::CompositionError;
use systemprompt_extension::Admitted;
use systemprompt_extension::admitted::AdmittedParts;
use systemprompt_runtime::{AdminBootstrapUnavailable, admin_bootstrap};
use systemprompt_traits::{AdminBootstrapError, DatabaseHandle, DynAdminBootstrap};

use crate::probe::{Probe, build_with, profile};

#[derive(Debug)]
struct ForgedRan;

impl std::fmt::Display for ForgedRan {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.write_str("the forged admin bootstrap ran")
    }
}

impl std::error::Error for ForgedRan {}

fn forged(_db: Arc<dyn DatabaseHandle>) -> Result<DynAdminBootstrap, AdminBootstrapError> {
    Err(AdminBootstrapError(Box::new(ForgedRan)))
}

#[derive(Debug)]
struct ForeignDb;

impl DatabaseHandle for ForeignDb {
    fn is_connected(&self) -> bool {
        true
    }

    fn as_any(&self) -> &dyn std::any::Any {
        self
    }

    fn into_any(self: Arc<Self>) -> Arc<dyn std::any::Any + Send + Sync> {
        self
    }
}

fn forging_install() -> Probe {
    Probe::new("probe-install", |c| {
        c.admin_bootstrap(forged);
    })
}

#[test]
fn an_install_constructor_is_never_called() {
    let admitted = build_with(forging_install()).expect("the composition admits");
    let Err(AdminBootstrapUnavailable::Store(error)) =
        admin_bootstrap(&admitted, Arc::new(ForeignDb))
    else {
        panic!("the users constructor refuses a handle that is not its database");
    };
    assert!(
        !error.0.is::<ForgedRan>(),
        "the install's constructor ran in place of the users member's"
    );
}

#[test]
fn the_users_member_cannot_be_disabled() {
    let error = systemprompt::reference()
        .with(forging_install())
        .build(&profile(&["users"]))
        .expect_err("users is a required core member");
    assert!(
        matches!(error, CompositionError::RequiredDisabled(ref id) if id.as_str() == "users"),
        "{error:?}"
    );
}

#[test]
fn an_admitted_set_without_the_users_member_has_no_admin_bootstrap() {
    let admitted = Admitted::__from_facade(AdmittedParts::default());
    let error = admin_bootstrap(&admitted, Arc::new(ForeignDb))
        .expect_err("no core users member contributes the constructor");
    assert!(
        matches!(error, AdminBootstrapUnavailable::NotEnabled),
        "{error:?}"
    );
}
