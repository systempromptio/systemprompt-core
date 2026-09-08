use std::cell::RefCell;
use std::sync::Mutex;
use systemprompt_bridge::config::store::{
    ConfigStore, ConfigStoreError, ManagedPolicyRead, PolicyDocument, PolicyDocumentValue,
    PolicyHive, PolicyWrite, verified,
};

#[derive(Default)]
struct Fake {
    machine: Option<PolicyDocument>,
    user: Mutex<RefCell<PolicyDocument>>,
    ignore_write: bool,
    ignore_delete: bool,
    deny_machine_read: bool,
}

impl ConfigStore for Fake {
    fn policy_key_exists(&self, hive: PolicyHive) -> Result<bool, ConfigStoreError> {
        if hive == PolicyHive::Machine && self.deny_machine_read {
            return Err(ConfigStoreError::Backend("HKLM read denied".into()));
        }
        Ok(hive == PolicyHive::User || self.machine.is_some())
    }
    fn read_policy_document(
        &self,
        hive: PolicyHive,
        _: &[&str],
    ) -> Result<PolicyDocument, ConfigStoreError> {
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
        entries: &[(String, PolicyDocumentValue)],
    ) -> Result<(), ConfigStoreError> {
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
        verified::apply(&store, PolicyHive::User, &values("desired")),
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
        verified::apply(&store, PolicyHive::User, &values("desired")),
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
        verified::apply(&store, PolicyHive::User, &values("desired")),
        Err(ConfigStoreError::HiveConflict { .. })
    ));
}
#[test]
fn inaccessible_machine_policy_cannot_fall_back_to_user_policy() {
    let store = Fake {
        deny_machine_read: true,
        ..Fake::default()
    };
    assert!(verified::apply(&store, PolicyHive::User, &values("desired")).is_err());
}
#[test]
fn receipts_distinguish_actual_write_from_existing_policy() {
    let store = Fake::default();
    assert_eq!(
        verified::apply(&store, PolicyHive::User, &values("desired"))
            .unwrap()
            .outcome(),
        PolicyWrite::Written(PolicyHive::User)
    );
    assert_eq!(
        verified::apply(&store, PolicyHive::User, &values("desired"))
            .unwrap()
            .outcome(),
        PolicyWrite::AlreadyVerified(PolicyHive::User)
    );
    let machine = Fake {
        machine: Some(values("desired").into_iter().collect()),
        ..Fake::default()
    };
    assert_eq!(
        verified::apply(&machine, PolicyHive::User, &values("desired"))
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
        verified::remove_values(&store, PolicyHive::User, &["managedMcpServers"]),
        Err(ConfigStoreError::VerifyMismatch { .. })
    ));
}
