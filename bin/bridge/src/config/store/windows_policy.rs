//! Verified writes to the two Windows policy keys: Claude's
//! `SOFTWARE\Policies\Claude` and the bridge's own signing-trust key.
//!
//! An unelevated write lands in HKCU, which Claude honours only while no HKLM
//! key exists; so a per-user write is refused with `HiveConflict` when the
//! machine key disagrees and reported as `SatisfiedByMachine` when it already
//! says the same thing.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use windows_sys::Win32::System::Registry::HKEY;

use super::verified::{self, PolicyReceipt};
use super::{
    ConfigStoreError, PolicyDocumentValue, PolicyHive, PolicyWrite, bridge_policy_subkey, hive_for,
    windows_registry, windows_registry_write,
};

pub(crate) fn write_managed_claude_policy(
    elevated: bool,
    entries: &[(String, String)],
) -> Result<PolicyReceipt, ConfigStoreError> {
    verified::apply(
        &windows_registry::WindowsRegistryStore,
        hive_for(elevated),
        &typed_strings(entries),
    )
}

pub(crate) fn write_bridge_policy(
    elevated: bool,
    entries: &[(String, String)],
) -> Result<PolicyReceipt, ConfigStoreError> {
    let subkey = bridge_policy_subkey();
    let names: Vec<String> = entries.iter().map(|(name, _)| name.clone()).collect();
    if !elevated && windows_registry::key_exists(PolicyHive::Machine, &subkey)? {
        ensure_machine_agrees(&subkey, entries)?;
        return Ok(PolicyReceipt::new(
            PolicyWrite::SatisfiedByMachine,
            subkey,
            names,
        ));
    }
    let hive = hive_for(elevated);
    let drifted = entries.iter().try_fold(false, |drifted, (name, value)| {
        let stored = windows_registry::read_string(windows_registry::hkey(hive), &subkey, name)?;
        Ok::<_, ConfigStoreError>(drifted || stored.as_ref() != Some(value))
    })?;
    if drifted {
        windows_registry_write::write_values_at(hive, &subkey, &typed_strings(entries))?;
    }
    // Why: a machine key that appeared between the first check and the write
    // shadows the per-user value just written, so the write must not report
    // success.
    if !elevated && windows_registry::key_exists(PolicyHive::Machine, &subkey)? {
        ensure_machine_agrees(&subkey, entries)?;
    }
    let outcome = if drifted {
        PolicyWrite::Written(hive)
    } else {
        PolicyWrite::AlreadyVerified(hive)
    };
    Ok(PolicyReceipt::new(outcome, subkey, names))
}

fn ensure_machine_agrees(
    subkey: &str,
    entries: &[(String, String)],
) -> Result<(), ConfigStoreError> {
    let machine = windows_registry::hkey(PolicyHive::Machine);
    let mut differing = Vec::new();
    for (name, value) in entries {
        if windows_registry::read_string(machine, subkey, name)?.as_ref() != Some(value) {
            differing.push(name.clone());
        }
    }
    if differing.is_empty() {
        Ok(())
    } else {
        Err(ConfigStoreError::HiveConflict {
            subkey: subkey.to_owned(),
            differing,
        })
    }
}

fn typed_strings(entries: &[(String, String)]) -> Vec<(String, PolicyDocumentValue)> {
    entries
        .iter()
        .map(|(n, v)| (n.clone(), PolicyDocumentValue::Str(v.clone())))
        .collect()
}

pub(crate) fn clear_managed_claude_policy(
    elevated: bool,
    names: &[&str],
) -> Result<usize, ConfigStoreError> {
    verified::remove_values(
        &windows_registry::WindowsRegistryStore,
        hive_for(elevated),
        names,
    )
}

pub(crate) fn read_registry_string(
    hive: HKEY,
    subkey: &str,
    name: &str,
) -> Result<Option<String>, ConfigStoreError> {
    windows_registry::read_string(hive, subkey, name)
}
