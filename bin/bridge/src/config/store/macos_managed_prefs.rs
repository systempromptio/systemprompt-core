//! macOS managed-preferences (`/Library/Managed Preferences`) policy store.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]
#![allow(
    unsafe_code,
    reason = "CoreFoundation preferences FFI for managed app domain"
)]

use std::collections::BTreeMap;

use std::ffi::c_void;

use core_foundation::array::CFArray;
use core_foundation::base::{CFType, TCFType};
use core_foundation::boolean::CFBoolean;
use core_foundation::dictionary::CFDictionary;
use core_foundation::number::CFNumber;
use core_foundation::string::CFString;
use core_foundation_sys::preferences::{CFPreferencesAppSynchronize, CFPreferencesCopyAppValue};
use core_foundation_sys::propertylist::CFPropertyListRef;

use super::{
    ConfigStore, ConfigStoreError, ManagedPolicyRead, PolicyDocument, PolicyDocumentValue,
    PolicyHive, PolicyTarget,
};

const POLICY_DOMAIN: &str = "com.anthropic.claudefordesktop";

pub(super) struct MacOsManagedPrefsStore;

impl ConfigStore for MacOsManagedPrefsStore {
    fn policy_key_exists(
        &self,
        hive: PolicyHive,
        target: PolicyTarget,
    ) -> Result<bool, ConfigStoreError> {
        claude_only(target)?;
        let path = super::macos_plist_store::plist_path(hive).ok_or_else(|| {
            ConfigStoreError::Backend("per-user policy path unresolvable".to_owned())
        })?;
        path.try_exists()
            .map_err(|e| ConfigStoreError::Backend(format!("{}: {e}", path.display())))
    }
    fn read_managed_policy(&self, key: &str) -> Result<Option<String>, ConfigStoreError> {
        synchronize_domain()?;
        copy_app_string(key)
    }

    fn read_managed_policy_keys(
        &self,
        keys: &[&str],
    ) -> Result<ManagedPolicyRead, ConfigStoreError> {
        synchronize_domain()?;
        let mut values: BTreeMap<String, String> = BTreeMap::new();
        for key in keys {
            if let Some(v) = copy_app_string(key)? {
                values.insert((*key).to_owned(), v);
            }
        }
        let source = if values.is_empty() {
            None
        } else {
            Some(format!(
                "/Library/Managed Preferences/{POLICY_DOMAIN}.plist"
            ))
        };
        Ok(ManagedPolicyRead { source, values })
    }

    fn read_policy_document(
        &self,
        hive: PolicyHive,
        target: PolicyTarget,
        keys: &[&str],
    ) -> Result<PolicyDocument, ConfigStoreError> {
        claude_only(target)?;
        super::macos_plist_store::read_document(hive, keys)
    }

    fn write_policy_values(
        &self,
        hive: PolicyHive,
        target: PolicyTarget,
        entries: &[(String, PolicyDocumentValue)],
    ) -> Result<(), ConfigStoreError> {
        claude_only(target)?;
        super::macos_plist_store::write_values(hive, entries)
    }

    fn delete_policy_values(
        &self,
        hive: PolicyHive,
        target: PolicyTarget,
        names: &[&str],
    ) -> Result<usize, ConfigStoreError> {
        claude_only(target)?;
        super::macos_plist_store::delete_values(hive, names)
    }

    fn delete_policy_key(&self, hive: PolicyHive) -> Result<bool, ConfigStoreError> {
        super::macos_plist_store::delete_key(hive)
    }
}

// Why: on macOS the bridge's signing trust is a managed profile installed
// through `install --apply`, not a value in the Claude preferences domain.
fn claude_only(target: PolicyTarget) -> Result<(), ConfigStoreError> {
    match target {
        PolicyTarget::Claude => Ok(()),
        PolicyTarget::Bridge => Err(ConfigStoreError::Backend(
            "the bridge signing-trust key is a managed profile on macOS".to_owned(),
        )),
    }
}

fn synchronize_domain() -> Result<(), ConfigStoreError> {
    let domain = CFString::new(POLICY_DOMAIN);
    // SAFETY: `domain` is a live `CFString` whose ref is valid for the call's
    // duration.
    if unsafe { CFPreferencesAppSynchronize(domain.as_concrete_TypeRef()) } == 0 {
        return Err(ConfigStoreError::Backend(format!(
            "synchronize managed preferences {POLICY_DOMAIN} failed"
        )));
    }
    Ok(())
}

fn copy_app_string(key: &str) -> Result<Option<String>, ConfigStoreError> {
    let key_cf = CFString::new(key);
    let domain_cf = CFString::new(POLICY_DOMAIN);
    // SAFETY: `key_cf` and `domain_cf` are live `CFString`s; the returned ref
    // follows the Copy rule and is wrapped below, which releases it.
    let raw: CFPropertyListRef = unsafe {
        CFPreferencesCopyAppValue(
            key_cf.as_concrete_TypeRef(),
            domain_cf.as_concrete_TypeRef(),
        )
    };
    if raw.is_null() {
        return Ok(None);
    }
    // SAFETY: `raw` is non-null and a valid CoreFoundation type ref obtained
    // under the Copy rule, so ownership transfers to the wrapper.
    let value: CFType = unsafe { TCFType::wrap_under_create_rule(raw.cast()) };
    let json = cf_to_json(&value).ok_or_else(|| {
        ConfigStoreError::Backend(format!(
            "{POLICY_DOMAIN}: unsupported policy value at {key}"
        ))
    })?;
    Ok(Some(match json {
        serde_json::Value::String(s) => s,
        other => other.to_string(),
    }))
}

fn cf_to_json(value: &CFType) -> Option<serde_json::Value> {
    if let Some(s) = value.downcast::<CFString>() {
        return Some(serde_json::Value::String(s.to_string()));
    }
    if let Some(b) = value.downcast::<CFBoolean>() {
        return Some(serde_json::Value::Bool(b == CFBoolean::true_value()));
    }
    if let Some(n) = value.downcast::<CFNumber>() {
        if let Some(i) = n.to_i64() {
            return Some(serde_json::Value::from(i));
        }
        return n
            .to_f64()
            .and_then(serde_json::Number::from_f64)
            .map(serde_json::Value::Number);
    }
    if let Some(array) = value.downcast::<CFArray<*const c_void>>() {
        let mut out = Vec::with_capacity(array.len().try_into().unwrap_or(0));
        for item in array.iter() {
            // SAFETY: the element is a borrowed Get-rule ref owned by the
            // array, which outlives the wrapper built from it here.
            let item: CFType = unsafe { TCFType::wrap_under_get_rule((*item).cast()) };
            out.push(cf_to_json(&item)?);
        }
        return Some(serde_json::Value::Array(out));
    }
    if let Some(dict) = value.downcast::<CFDictionary<*const c_void, *const c_void>>() {
        let mut out = serde_json::Map::new();
        let (keys, values) = dict.get_keys_and_values();
        for (k, v) in keys.into_iter().zip(values) {
            // SAFETY: `k` comes from `get_keys_and_values`, which returns a
            // borrowed Get-rule ref valid for the dictionary's lifetime.
            let key: CFType = unsafe { TCFType::wrap_under_get_rule(k.cast()) };
            // SAFETY: `v` is the matching value ref from the same call, with
            // the same borrowed Get-rule lifetime.
            let val: CFType = unsafe { TCFType::wrap_under_get_rule(v.cast()) };
            out.insert(key.downcast::<CFString>()?.to_string(), cf_to_json(&val)?);
        }
        return Some(serde_json::Value::Object(out));
    }
    None
}
