//! Verified writes to Claude's `SOFTWARE\Policies\Claude` key through the
//! registry store; the bridge's own signing-trust key goes through the same
//! `verified::apply` with `PolicyTarget::Bridge`.
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
use super::{ConfigStoreError, PolicyDocumentValue, PolicyTarget, hive_for, windows_registry};

pub(crate) fn write_managed_claude_policy(
    elevated: bool,
    entries: &[(String, String)],
) -> Result<PolicyReceipt, ConfigStoreError> {
    verified::apply(
        &windows_registry::WindowsRegistryStore,
        hive_for(elevated),
        PolicyTarget::Claude,
        &PolicyDocumentValue::strings(entries),
    )
}

pub(crate) fn clear_managed_claude_policy(
    elevated: bool,
    names: &[&str],
) -> Result<usize, ConfigStoreError> {
    verified::remove_values(
        &windows_registry::WindowsRegistryStore,
        hive_for(elevated),
        PolicyTarget::Claude,
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
