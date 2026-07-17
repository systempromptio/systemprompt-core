//! Windows certificate-store (`HCERTSTORE`) device-certificate source.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

#![allow(
    unsafe_code,
    reason = "Windows CryptoAPI / NCrypt FFI for machine-key device cert"
)]

use super::{DeviceCert, DeviceCertSource, KeystoreError, sha256_der};
use std::mem::ManuallyDrop;
use std::{env, ptr};
use windows_sys::Win32::Security::Cryptography::{
    CERT_CONTEXT, CertCloseStore, CertEnumCertificatesInStore, CertFreeCertificateContext,
    CertOpenSystemStoreW, HCERTSTORE,
};

pub(super) struct WindowsKeystore {
    match_fingerprint: Option<String>,
}

impl WindowsKeystore {
    pub(super) fn new() -> Self {
        Self {
            match_fingerprint: env::var(crate::brand::brand().env("DEVICE_CERT_SHA256")).ok(),
        }
    }
}

struct StoreHandle(HCERTSTORE);

impl StoreHandle {
    fn open_my() -> Result<Self, KeystoreError> {
        let name: Vec<u16> = "MY\0".encode_utf16().collect();
        // SAFETY: `name` is a live NUL-terminated UTF-16 store name; a null provider
        // handle selects the default.
        let handle = unsafe { CertOpenSystemStoreW(0, name.as_ptr()) };
        if handle.is_null() {
            return Err(KeystoreError::Other(
                "CertOpenSystemStoreW(MY) returned NULL".into(),
            ));
        }
        Ok(Self(handle))
    }
}

impl Drop for StoreHandle {
    fn drop(&mut self) {
        // SAFETY: `self.0` is a store handle this `StoreHandle` exclusively owns and
        // closes once.
        unsafe {
            CertCloseStore(self.0, 0);
        }
    }
}

struct CertHandle(*const CERT_CONTEXT);

impl Drop for CertHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: `self.0` is a non-null cert context this `CertHandle` exclusively
            // owns.
            unsafe {
                CertFreeCertificateContext(self.0);
            }
        }
    }
}

impl DeviceCertSource for WindowsKeystore {
    fn load(&self) -> Result<DeviceCert, KeystoreError> {
        let store = StoreHandle::open_my()?;
        let mut prev: *const CERT_CONTEXT = ptr::null();

        loop {
            // SAFETY: `store.0` is a live store handle and `prev` is either null or a
            // context from a prior iteration that this call takes ownership of
            // and frees.
            let next = unsafe { CertEnumCertificatesInStore(store.0, prev) };
            if next.is_null() {
                break;
            }
            let current = ManuallyDrop::new(CertHandle(next));
            let der = cert_encoded_bytes(current.0);
            let fingerprint = match sha256_der(&der) {
                Ok(fp) => fp,
                Err(e) => {
                    drop(ManuallyDrop::into_inner(current));
                    return Err(e);
                },
            };
            if self
                .match_fingerprint
                .as_deref()
                .is_none_or(|want| want.eq_ignore_ascii_case(fingerprint.as_str()))
            {
                drop(ManuallyDrop::into_inner(current));
                return Ok(DeviceCert { fingerprint });
            }
            prev = next;
        }

        Err(KeystoreError::NotFound(
            self.match_fingerprint.as_deref().map_or_else(
                || "MY certificate store is empty".to_owned(),
                |fp| format!("no certificate in MY store matched SHA-256 {fp}"),
            ),
        ))
    }
}

fn cert_encoded_bytes(ctx: *const CERT_CONTEXT) -> Vec<u8> {
    if ctx.is_null() {
        return Vec::new();
    }
    // SAFETY: `ctx` is non-null and points to a live `CERT_CONTEXT`;
    // `pbCertEncoded` and `cbCertEncoded` describe a contiguous DER buffer
    // owned by that context, read here without outliving it.
    unsafe {
        let len = (*ctx).cbCertEncoded as usize;
        let ptr = (*ctx).pbCertEncoded;
        if ptr.is_null() || len == 0 {
            return Vec::new();
        }
        std::slice::from_raw_parts(ptr, len).to_vec()
    }
}

pub fn platform_source() -> Box<dyn DeviceCertSource> {
    Box::new(WindowsKeystore::new())
}
