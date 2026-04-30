#![cfg(target_os = "macos")]
#![allow(unsafe_code)]

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
                values.insert((*key).to_string(), v);
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
    unsafe { CFPreferencesAppSynchronize(domain.as_concrete_TypeRef()) };
}

fn copy_app_string(key: &str) -> Option<String> {
    let key_cf = CFString::new(key);
    let domain_cf = CFString::new(POLICY_DOMAIN);
    let raw: CFPropertyListRef = unsafe {
        CFPreferencesCopyAppValue(key_cf.as_concrete_TypeRef(), domain_cf.as_concrete_TypeRef())
    };
    if raw.is_null() {
        return None;
    }
    let type_id = unsafe { CFGetTypeID(raw as CFTypeRef) };
    if type_id != unsafe { CFStringGetTypeID() } {
        unsafe { release_unknown(raw) };
        return None;
    }
    let cf_string: CFString =
        unsafe { TCFType::wrap_under_create_rule(raw as CFStringRef) };
    Some(cf_string.to_string())
}

unsafe fn release_unknown(raw: CFPropertyListRef) {
    use core_foundation_sys::base::CFRelease;
    unsafe { CFRelease(raw as CFTypeRef) };
}
