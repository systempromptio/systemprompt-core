//! The governance slots: contributed authz hooks compose core first into one
//! hook, and the role directory is single-valued.

use std::sync::Arc;

use async_trait::async_trait;
use systemprompt_extension::{
    FactoryError, Resolved, Slot, SlotEntry, SlotError, SlotOrder, StandingTag,
};
use systemprompt_governance::authz::{
    AllowAllHook, AuthzError, NullAuditSink, RoleDirectory, SharedAuthzHook, SharedRoleDirectory,
};
use systemprompt_governance::contract::{
    AuthzHookBuilder, AuthzHookSlot, RoleDirectoryBuilder, RoleDirectorySlot,
};
use systemprompt_identifiers::{ExtensionId, UserId};

use crate::probe::kernel;

fn entry<B>(owner: &str, standing: StandingTag, builder: B) -> SlotEntry<B> {
    SlotEntry {
        owner: ExtensionId::new(owner),
        standing,
        builder,
    }
}

fn allow(_: &Resolved<'_>) -> Result<SharedAuthzHook, FactoryError> {
    Ok(Arc::new(AllowAllHook::new(Arc::new(NullAuditSink))))
}

#[derive(Debug)]
struct NoRoles;

#[async_trait]
impl RoleDirectory for NoRoles {
    async fn unknown_roles(&self, candidates: &[String]) -> Result<Vec<String>, AuthzError> {
        Ok(candidates.to_vec())
    }

    async fn roles_of(&self, _user: &UserId) -> Result<Vec<String>, AuthzError> {
        Ok(Vec::new())
    }
}

fn directory(_: &Resolved<'_>) -> Result<SharedRoleDirectory, FactoryError> {
    Ok(Arc::new(NoRoles))
}

#[test]
fn governance_slots_are_core_first() {
    assert_eq!(AuthzHookSlot::ORDER, SlotOrder::CoreFirst);
    assert_eq!(RoleDirectorySlot::ORDER, SlotOrder::CoreFirst);
}

#[test]
fn no_contributed_hook_builds_none() {
    let hooks = AuthzHookSlot::build(Vec::new(), &kernel().resolved()).expect("empty builds");
    assert!(hooks.hook().is_none());
}

#[test]
fn two_contributed_hooks_compose_into_one() {
    let mut core = AuthzHookBuilder::default();
    core.hook(allow);
    let mut install = AuthzHookBuilder::default();
    install.hook(allow);
    let hooks = AuthzHookSlot::build(
        vec![
            entry("core", StandingTag::Core, core),
            entry("install", StandingTag::Install, install),
        ],
        &kernel().resolved(),
    )
    .expect("the hooks build");
    let hook = hooks.hook().expect("a composed hook");
    assert!(format!("{hook:?}").contains("CompositeAuthzHook"));
}

#[test]
fn a_second_role_directory_is_refused() {
    let mut first = RoleDirectoryBuilder::default();
    first.directory(directory);
    let mut second = RoleDirectoryBuilder::default();
    second.directory(directory);
    let err = RoleDirectorySlot::build(
        vec![
            entry("users", StandingTag::Core, first),
            entry("acme", StandingTag::Install, second),
        ],
        &kernel().resolved(),
    )
    .expect_err("a second directory is refused");
    assert!(
        matches!(err, SlotError::MultipleEntries { count: 2, .. }),
        "unexpected error: {err:?}"
    );
}

#[test]
fn one_role_directory_builds_as_the_negative_control() {
    let mut only = RoleDirectoryBuilder::default();
    only.directory(directory);
    let built = RoleDirectorySlot::build(
        vec![entry("users", StandingTag::Core, only)],
        &kernel().resolved(),
    )
    .expect("one directory builds");
    assert!(built.directory().is_some());
}
