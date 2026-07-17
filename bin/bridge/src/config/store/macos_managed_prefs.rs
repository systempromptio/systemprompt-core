//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![cfg(target_os = "macos")]
#![allow(
    unsafe_code,
    reason = "CoreFoundation preferences FFI for managed app domain"
)]

use std::collections::BTreeMap;

use core_foundation::base::{CFTypeRef, TCFType};
use core_foundation::string::{CFString, CFStringRef};
use core_foundation_sys::base::CFGetTypeID;
use core_foundation_sys::preferences::{CFPreferencesAppSynchronize, CFPreferencesCopyAppValue};
use core_foundation_sys::propertylist::CFPropertyListRef;
use core_foundation_sys::string::CFStringGetTypeID;

use super::{ConfigStore, ConfigStoreError, ManagedPolicyRead};

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
}

fn synchronize_domain() {
    let domain = CFString::new(POLICY_DOMAIN);
    // SAFETY: `domain` is a live `CFString` whose ref is valid for the call's
    // duration.
    unsafe { CFPreferencesAppSynchronize(domain.as_concrete_TypeRef()) };
}

fn copy_app_string(key: &str) -> Option<String> {
    let key_cf = CFString::new(key);
    let domain_cf = CFString::new(POLICY_DOMAIN);
    // SAFETY: `key_cf` and `domain_cf` are live `CFString`s; the returned ref
    // follows the Copy rule and is released or wrapped below.
    let raw: CFPropertyListRef = unsafe {
        CFPreferencesCopyAppValue(
            key_cf.as_concrete_TypeRef(),
            domain_cf.as_concrete_TypeRef(),
        )
    };
    if raw.is_null() {
        return None;
    }
    // SAFETY: `raw` is non-null and a valid CoreFoundation type ref.
    let type_id = unsafe { CFGetTypeID(raw as CFTypeRef) };
    // SAFETY: `CFStringGetTypeID` is a pure accessor with no preconditions.
    if type_id != unsafe { CFStringGetTypeID() } {
        // SAFETY: `raw` is a non-null Copy-rule ref this branch owns and must release.
        unsafe { release_unknown(raw) };
        return None;
    }
    // SAFETY: `raw` is confirmed to be a `CFStringRef` obtained under the Copy
    // rule, so ownership transfers to the wrapper.
    let cf_string: CFString = unsafe { TCFType::wrap_under_create_rule(raw as CFStringRef) };
    Some(cf_string.to_string())
}

unsafe fn release_unknown(raw: CFPropertyListRef) {
    use core_foundation_sys::base::CFRelease;
    // SAFETY: `raw` is a non-null Copy-rule ref the caller owns and releases
    // exactly once.
    unsafe { CFRelease(raw as CFTypeRef) };
}
