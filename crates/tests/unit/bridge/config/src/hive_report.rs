use systemprompt_bridge::config::ConfigWriteError;
use systemprompt_bridge::config::store::{
    ConfigStore, ConfigStoreError, HiveReport, ManagedPolicyRead, PolicyDocument,
    PolicyDocumentValue, PolicyHive, PolicyTarget, hive_report,
};

struct Hives {
    machine: PolicyDocument,
    user: PolicyDocument,
    deny_user: bool,
}

impl ConfigStore for Hives {
    fn policy_key_exists(&self, _: PolicyHive, _: PolicyTarget) -> Result<bool, ConfigStoreError> {
        unreachable!()
    }
    fn read_policy_document(
        &self,
        hive: PolicyHive,
        _: PolicyTarget,
        _: &[&str],
    ) -> Result<PolicyDocument, ConfigStoreError> {
        match hive {
            PolicyHive::Machine => Ok(self.machine.clone()),
            PolicyHive::User if self.deny_user => {
                Err(ConfigStoreError::Backend("HKCU read denied".into()))
            },
            PolicyHive::User => Ok(self.user.clone()),
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
        _: PolicyTarget,
        _: &[(String, PolicyDocumentValue)],
    ) -> Result<(), ConfigStoreError> {
        unreachable!()
    }
    fn delete_policy_values(
        &self,
        _: PolicyHive,
        _: PolicyTarget,
        _: &[&str],
    ) -> Result<usize, ConfigStoreError> {
        unreachable!()
    }
}

fn doc(url: &str) -> PolicyDocument {
    PolicyDocument::from([(
        "managedMcpServers".to_owned(),
        PolicyDocumentValue::Str(url.to_owned()),
    )])
}

fn report(machine: Option<&str>, user: Option<&str>, elevated: bool) -> HiveReport {
    hive_report(&hives(machine, user), elevated).expect("both hives readable")
}

fn foreign_owner(path: &str, owner: &str) -> String {
    ConfigWriteError::ForeignOwner {
        path: path.into(),
        owner: owner.into(),
    }
    .to_string()
}

fn hives(machine: Option<&str>, user: Option<&str>) -> Hives {
    Hives {
        machine: machine.map(doc).unwrap_or_default(),
        user: user.map(doc).unwrap_or_default(),
        deny_user: false,
    }
}

#[test]
fn a_machine_key_with_other_values_shadows_the_user_copy() {
    let report = report(Some("https://a"), Some("https://b"), false);
    assert_eq!(report, HiveReport::Shadowed);
    assert!(report.is_failure());
    assert!(
        report.detail().contains("administrator"),
        "{}",
        report.detail()
    );
}

#[test]
fn matching_hives_and_single_hives_are_healthy() {
    assert_eq!(
        report(Some("https://a"), Some("https://a"), false),
        HiveReport::Matching
    );
    assert_eq!(report(Some("https://a"), None, false), HiveReport::Machine);
    assert_eq!(report(None, Some("https://a"), false), HiveReport::User);
    for report in [HiveReport::Matching, HiveReport::Machine, HiveReport::User] {
        assert!(!report.is_failure() && !report.is_warning(), "{report:?}");
    }
}

#[test]
fn an_elevated_process_over_a_user_only_policy_warns_that_it_will_shadow_it() {
    let report = report(None, Some("https://a"), true);
    assert_eq!(report, HiveReport::UserBeneathElevatedWriter);
    assert!(report.is_warning());
    assert_eq!(
        hive_report(&hives(None, None), true).expect("readable"),
        HiveReport::Unwritten
    );
}

#[test]
fn an_unreadable_hive_is_an_error_not_a_guess() {
    let mut store = hives(Some("https://a"), Some("https://a"));
    store.deny_user = true;
    let err = hive_report(&store, false).unwrap_err();
    assert!(err.contains("HKCU read denied"), "{err}");
}

#[test]
fn a_foreign_owner_names_the_path_and_account_and_the_remedy() {
    let text = foreign_owner(
        "C:\\Users\\rxmar\\AppData\\Roaming\\astound",
        "S-1-5-21-1-2-3-500",
    );
    assert!(text.contains("AppData\\Roaming\\astound"), "{text}");
    assert!(text.contains("S-1-5-21-1-2-3-500"), "{text}");
    assert!(text.contains("repair it as administrator"), "{text}");
}
