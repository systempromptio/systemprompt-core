//! Applying a rendered Claude Desktop policy over a fake hive: a written
//! `managedMcpServers` replaces the stale value, an omitted one clears it.

use std::collections::BTreeMap;
use std::sync::Mutex;

use systemprompt_bridge::config::store::{
    ConfigStore, ConfigStoreError, ManagedPolicyRead, PolicyDocument, PolicyDocumentValue,
    PolicyHive, PolicyTarget,
};
use systemprompt_bridge::install::mdm::policy_apply::apply_claude_policy;

const STALE: &str = r#"[{"name":"atlassian","url":"http://127.0.0.1:48218/mcp/atlassian"}]"#;

#[derive(Default)]
struct Hive(Mutex<BTreeMap<String, PolicyDocumentValue>>);

impl Hive {
    fn with(name: &str, value: &str) -> Self {
        let hive = Self::default();
        hive.0
            .lock()
            .unwrap()
            .insert(name.to_owned(), PolicyDocumentValue::Str(value.to_owned()));
        hive
    }

    fn get(&self, name: &str) -> Option<PolicyDocumentValue> {
        self.0.lock().unwrap().get(name).cloned()
    }
}

impl ConfigStore for Hive {
    fn policy_key_exists(
        &self,
        hive: PolicyHive,
        _: PolicyTarget,
    ) -> Result<bool, ConfigStoreError> {
        Ok(hive == PolicyHive::Machine)
    }
    fn read_managed_policy(&self, _: &str) -> Result<Option<String>, ConfigStoreError> {
        unreachable!()
    }
    fn read_managed_policy_keys(&self, _: &[&str]) -> Result<ManagedPolicyRead, ConfigStoreError> {
        unreachable!()
    }
    fn read_policy_document(
        &self,
        _: PolicyHive,
        _: PolicyTarget,
        keys: &[&str],
    ) -> Result<PolicyDocument, ConfigStoreError> {
        let doc = self.0.lock().unwrap();
        Ok(keys
            .iter()
            .filter_map(|k| doc.get(*k).map(|v| ((*k).to_owned(), v.clone())))
            .collect())
    }
    fn write_policy_values(
        &self,
        _: PolicyHive,
        _: PolicyTarget,
        entries: &[(String, PolicyDocumentValue)],
    ) -> Result<(), ConfigStoreError> {
        self.0.lock().unwrap().extend(entries.iter().cloned());
        Ok(())
    }
    fn delete_policy_values(
        &self,
        _: PolicyHive,
        _: PolicyTarget,
        names: &[&str],
    ) -> Result<usize, ConfigStoreError> {
        let mut doc = self.0.lock().unwrap();
        Ok(names.iter().filter(|n| doc.remove(**n).is_some()).count())
    }
}

#[test]
fn a_written_server_list_replaces_the_stale_loopback_list() {
    let hive = Hive::with("managedMcpServers", STALE);
    let fresh = r#"[{"name":"atlassian","url":"http://127.0.0.1:48217/mcp/atlassian"}]"#;
    let values = [
        ("inferenceProvider", "REG_SZ", "gateway".to_owned()),
        ("managedMcpServers", "REG_SZ", fresh.to_owned()),
    ];
    apply_claude_policy(&hive, PolicyHive::Machine, &values).unwrap();
    assert_eq!(
        hive.get("managedMcpServers"),
        Some(PolicyDocumentValue::Str(fresh.to_owned()))
    );
}

#[test]
fn an_omitted_server_list_clears_the_stale_value() {
    let hive = Hive::with("managedMcpServers", STALE);
    let values = [("inferenceProvider", "REG_SZ", "gateway".to_owned())];
    apply_claude_policy(&hive, PolicyHive::Machine, &values).unwrap();
    assert_eq!(
        hive.get("managedMcpServers"),
        None,
        "a list every server was skipped from must not leave the old loopback entries in force"
    );
    assert_eq!(
        hive.get("inferenceProvider"),
        Some(PolicyDocumentValue::Str("gateway".to_owned()))
    );
}
