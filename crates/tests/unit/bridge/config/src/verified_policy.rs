use std::cell::RefCell;
use std::sync::Mutex;
use systemprompt_bridge::config::store::{
    ConfigStore, ConfigStoreError, ManagedPolicyRead, PolicyDocument, PolicyDocumentValue,
    PolicyHive, PolicyTarget, PolicyWrite, verified,
};

#[derive(Default)]
struct Fake {
    machine: Option<PolicyDocument>,
    user: Mutex<RefCell<PolicyDocument>>,
    ignore_write: bool,
    ignore_delete: bool,
    deny_machine_read: bool,
    targets: Mutex<Vec<PolicyTarget>>,
}

impl Fake {
    fn saw(&self, target: PolicyTarget) {
        self.targets.lock().unwrap().push(target);
    }

    fn targets_seen(&self) -> Vec<PolicyTarget> {
        self.targets.lock().unwrap().clone()
    }
}

impl ConfigStore for Fake {
    fn policy_key_exists(
        &self,
        hive: PolicyHive,
        target: PolicyTarget,
    ) -> Result<bool, ConfigStoreError> {
        self.saw(target);
        if hive == PolicyHive::Machine && self.deny_machine_read {
            return Err(ConfigStoreError::Backend("HKLM read denied".into()));
        }
        Ok(hive == PolicyHive::User || self.machine.is_some())
    }
    fn read_policy_document(
        &self,
        hive: PolicyHive,
        target: PolicyTarget,
        _: &[&str],
    ) -> Result<PolicyDocument, ConfigStoreError> {
        self.saw(target);
        match hive {
            PolicyHive::Machine => Ok(self.machine.clone().unwrap_or_default()),
            PolicyHive::User => Ok(self.user.lock().unwrap().borrow().clone()),
        }
    }
    fn read_managed_policy(&self, _: &str) -> Result<Option<String>, ConfigStoreError> {
        unreachable!()
    }
    fn read_managed_policy_keys(&self, _: &[&str]) -> Result<ManagedPolicyRead, ConfigStoreError> {
        unreachable!()
    }
    fn write_policy_values(
        &self,
        _: PolicyHive,
        target: PolicyTarget,
        entries: &[(String, PolicyDocumentValue)],
    ) -> Result<(), ConfigStoreError> {
        self.saw(target);
        if !self.ignore_write {
            self.user
                .lock()
                .unwrap()
                .borrow_mut()
                .extend(entries.iter().cloned());
        }
        Ok(())
    }
    fn delete_policy_values(
        &self,
        _: PolicyHive,
        _: PolicyTarget,
        names: &[&str],
    ) -> Result<usize, ConfigStoreError> {
        if !self.ignore_delete {
            for name in names {
                self.user.lock().unwrap().borrow_mut().remove(*name);
            }
        }
        Ok(names.len())
    }
    fn delete_policy_key(&self, _: PolicyHive) -> Result<bool, ConfigStoreError> {
        unreachable!()
    }
}
fn values(value: &str) -> Vec<(String, PolicyDocumentValue)> {
    vec![(
        "managedMcpServers".into(),
        PolicyDocumentValue::Str(value.into()),
    )]
}

#[test]
fn success_without_a_write_is_rejected() {
    let store = Fake {
        ignore_write: true,
        ..Fake::default()
    };
    assert!(matches!(
        verified::apply(
            &store,
            PolicyHive::User,
            PolicyTarget::Claude,
            &values("desired")
        ),
        Err(ConfigStoreError::VerifyMismatch { .. })
    ));
}
#[test]
fn matching_user_value_cannot_hide_conflicting_machine_policy() {
    let store = Fake {
        machine: Some(values("stale").into_iter().collect()),
        ..Fake::default()
    };
    store
        .user
        .lock()
        .unwrap()
        .borrow_mut()
        .extend(values("desired"));
    assert!(matches!(
        verified::apply(
            &store,
            PolicyHive::User,
            PolicyTarget::Claude,
            &values("desired")
        ),
        Err(ConfigStoreError::HiveConflict { .. })
    ));
}
#[test]
fn empty_machine_key_is_not_an_absent_key() {
    let store = Fake {
        machine: Some(PolicyDocument::new()),
        ..Fake::default()
    };
    assert!(matches!(
        verified::apply(
            &store,
            PolicyHive::User,
            PolicyTarget::Claude,
            &values("desired")
        ),
        Err(ConfigStoreError::HiveConflict { .. })
    ));
}
#[test]
fn inaccessible_machine_policy_cannot_fall_back_to_user_policy() {
    let store = Fake {
        deny_machine_read: true,
        ..Fake::default()
    };
    assert!(
        verified::apply(
            &store,
            PolicyHive::User,
            PolicyTarget::Claude,
            &values("desired")
        )
        .is_err()
    );
}
#[test]
fn receipts_distinguish_actual_write_from_existing_policy() {
    let store = Fake::default();
    assert_eq!(
        verified::apply(
            &store,
            PolicyHive::User,
            PolicyTarget::Claude,
            &values("desired")
        )
        .unwrap()
        .outcome(),
        PolicyWrite::Written(PolicyHive::User)
    );
    assert_eq!(
        verified::apply(
            &store,
            PolicyHive::User,
            PolicyTarget::Claude,
            &values("desired")
        )
        .unwrap()
        .outcome(),
        PolicyWrite::AlreadyVerified(PolicyHive::User)
    );
    let machine = Fake {
        machine: Some(values("desired").into_iter().collect()),
        ..Fake::default()
    };
    assert_eq!(
        verified::apply(
            &machine,
            PolicyHive::User,
            PolicyTarget::Claude,
            &values("desired")
        )
        .unwrap()
        .outcome(),
        PolicyWrite::SatisfiedByMachine
    );
}
#[test]
fn successful_delete_with_surviving_value_fails_verification() {
    let store = Fake {
        ignore_delete: true,
        ..Fake::default()
    };
    store
        .user
        .lock()
        .unwrap()
        .borrow_mut()
        .extend(values("stale"));
    assert!(matches!(
        verified::remove_values(
            &store,
            PolicyHive::User,
            PolicyTarget::Claude,
            &["managedMcpServers"]
        ),
        Err(ConfigStoreError::VerifyMismatch { .. })
    ));
}

fn bridge_subkey() -> String {
    systemprompt_bridge::config::store::bridge_policy_subkey()
}

#[test]
fn the_bridge_target_reaches_the_store_and_names_its_own_subkey() {
    // Why: the Claude policy key and the bridge's own signing-trust key share
    // one verified algorithm. A target that was dropped on the way to the
    // store would write Claude's key while reporting the bridge's.
    let store = Fake::default();
    let receipt = verified::apply(
        &store,
        PolicyHive::User,
        PolicyTarget::Bridge,
        &values("desired"),
    )
    .expect("the write verifies");

    assert_eq!(receipt.outcome(), PolicyWrite::Written(PolicyHive::User));
    assert_eq!(receipt.names(), ["managedMcpServers".to_owned()]);
    let described = receipt.describe();
    assert!(
        described.contains(&bridge_subkey()),
        "the receipt names the key it actually wrote: {described}"
    );
    let seen = store.targets_seen();
    assert!(
        !seen.is_empty() && seen.iter().all(|target| *target == PolicyTarget::Bridge),
        "every store call carried the bridge target: {seen:?}"
    );
}

#[test]
fn a_hive_conflict_on_the_bridge_target_names_the_bridge_key() {
    let store = Fake {
        machine: Some(values("stale").into_iter().collect()),
        ..Fake::default()
    };
    let err = verified::apply(
        &store,
        PolicyHive::User,
        PolicyTarget::Bridge,
        &values("desired"),
    )
    .expect_err("machine policy shadows the per-user write");
    match &err {
        ConfigStoreError::HiveConflict { subkey, differing } => {
            assert_eq!(subkey, &bridge_subkey());
            assert_eq!(differing, &["managedMcpServers".to_owned()]);
        },
        other => panic!("{other:?}"),
    }
    assert!(
        !err.to_string()
            .contains(systemprompt_bridge::cowork_compat::POLICY_SUBKEY),
        "a bridge conflict must not send the administrator to Claude's key: {err}"
    );
}

#[test]
fn a_verify_mismatch_on_the_bridge_target_names_the_bridge_key() {
    let store = Fake {
        ignore_write: true,
        ..Fake::default()
    };
    let err = verified::apply(
        &store,
        PolicyHive::User,
        PolicyTarget::Bridge,
        &values("desired"),
    )
    .expect_err("a write that does not read back is not a success");
    match &err {
        ConfigStoreError::VerifyMismatch { subkey, name, .. } => {
            assert_eq!(subkey, &bridge_subkey());
            assert_eq!(name, "managedMcpServers");
        },
        other => panic!("{other:?}"),
    }
}

#[test]
fn the_two_targets_address_different_keys() {
    // Why: the negative control for the three tests above — if both targets
    // resolved to the same subkey, none of them would be checking anything.
    assert_ne!(
        bridge_subkey(),
        systemprompt_bridge::cowork_compat::POLICY_SUBKEY
    );
}
