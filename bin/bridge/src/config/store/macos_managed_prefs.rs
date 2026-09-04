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
    PolicyHive,
};

const POLICY_DOMAIN: &str = "com.anthropic.claudefordesktop";

pub(super) struct MacOsManagedPrefsStore;

impl ConfigStore for MacOsManagedPrefsStore {
    fn read_managed_policy(&self, key: &str) -> Result<Option<String>, ConfigStoreError> {
        synchronize_domain();
        Ok(copy_app_string(key))
    }

    fn read_managed_policy_keys(
        &self,
        keys: &[&str],
    ) -> Result<ManagedPolicyRead, ConfigStoreError> {
        synchronize_domain();
        let mut values: BTreeMap<String, String> = BTreeMap::new();
        for key in keys {
            if let Some(v) = copy_app_string(key) {
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
        keys: &[&str],
    ) -> Result<PolicyDocument, ConfigStoreError> {
        super::macos_plist_store::read_document(hive, keys)
    }

    fn write_policy_values(
        &self,
        hive: PolicyHive,
        entries: &[(String, PolicyDocumentValue)],
    ) -> Result<(), ConfigStoreError> {
        super::macos_plist_store::write_values(hive, entries)
    }

    fn delete_policy_values(
        &self,
        hive: PolicyHive,
        names: &[&str],
    ) -> Result<usize, ConfigStoreError> {
        super::macos_plist_store::delete_values(hive, names)
    }

    fn delete_policy_key(&self, hive: PolicyHive) -> Result<bool, ConfigStoreError> {
        super::macos_plist_store::delete_key(hive)
    }
}

fn synchronize_domain() {
    let domain = CFString::new(POLICY_DOMAIN);
    // SAFETY: `domain` is a live `CFString` whose ref is valid for the call's
    // duration.
    unsafe { CFPreferencesAppSynchronize(domain.as_concrete_TypeRef()) };
}

// Why: a managed policy value is any property-list type, but this only ever
// understood `CFString` and returned `None` for everything else. The two keys
// Cowork needs most — `allowedWorkspaceFolders` and `managedMcpServers` — are
// arrays, so a fully-provisioned Mac reported them missing and `validate`
// failed on a machine whose policy was correct. The callers already expect
// JSON for those (`managedMcpServers == "[]"` is read as "none in manifest"),
// so serialise rather than widen the callers.
fn copy_app_string(key: &str) -> Option<String> {
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
        return None;
    }
    // SAFETY: `raw` is non-null and a valid CoreFoundation type ref obtained
    // under the Copy rule, so ownership transfers to the wrapper.
    let value: CFType = unsafe { TCFType::wrap_under_create_rule(raw.cast()) };
    match cf_to_json(&value)? {
        // A string stays a bare string: callers compare these to plain values
        // like "true" or a URL, and quoting them would break every match.
        serde_json::Value::String(s) => Some(s),
        other => Some(other.to_string()),
    }
}

// Why: recursion is over `CFType` rather than raw refs so every element is
// released by its wrapper. An unrepresentable leaf (data, date) collapses the
// whole value to `None` — reporting a policy we cannot faithfully render as
// absent is safer than reporting a lossy rendering as present.
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
        return n.to_f64().and_then(serde_json::Number::from_f64).map(serde_json::Value::Number);
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
            // SAFETY: both refs come from `get_keys_and_values`, which returns
            // borrowed Get-rule refs valid for the dictionary's lifetime.
            let key: CFType = unsafe { TCFType::wrap_under_get_rule(k.cast()) };
            let val: CFType = unsafe { TCFType::wrap_under_get_rule(v.cast()) };
            out.insert(key.downcast::<CFString>()?.to_string(), cf_to_json(&val)?);
        }
        return Some(serde_json::Value::Object(out));
    }
    None
}

