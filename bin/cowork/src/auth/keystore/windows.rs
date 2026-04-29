#![allow(unsafe_code)]

use super::{DeviceCert, DeviceCertSource, KeystoreError, sha256_der};
use std::{env, ptr};
use windows_sys::Win32::Security::Cryptography::{
    CERT_CONTEXT, CertCloseStore, CertEnumCertificatesInStore, CertFreeCertificateContext,
    CertOpenSystemStoreW, HCERTSTORE,
};

pub struct WindowsKeystore {
    match_fingerprint: Option<String>,
}

impl WindowsKeystore {
    pub fn new() -> Self {
        Self {
            match_fingerprint: env::var("SP_COWORK_DEVICE_CERT_SHA256").ok(),
        }
    }
}

struct StoreHandle(HCERTSTORE);

impl StoreHandle {
    fn open_my() -> Result<Self, KeystoreError> {
        let name: Vec<u16> = "MY\0".encode_utf16().collect();
        // SAFETY: `name` is nul-terminated UTF-16 and lives until the call returns.
        // CertOpenSystemStoreW does not retain the pointer.
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
        // SAFETY: self.0 was returned by a successful CertOpenSystemStoreW and has not
        // been closed elsewhere; CertCloseStore is the matching deallocator.
        unsafe {
            CertCloseStore(self.0, 0);
        }
    }
}

struct CertHandle(*const CERT_CONTEXT);

impl Drop for CertHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            // SAFETY: self.0 was returned by CertEnumCertificatesInStore (or NULL, checked
            // above); CertFreeCertificateContext is the matching deallocator.
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
            // SAFETY: store.0 is a live HCERTSTORE owned by `store` for the duration of
            // this loop; `prev` is either null (first iteration) or a context
            // returned by a prior call to this same function and not yet freed.
            let next = unsafe { CertEnumCertificatesInStore(store.0, prev) };
            if next.is_null() {
                break;
            }
            let current = CertHandle(next);
            let der = cert_encoded_bytes(current.0);
            let fingerprint = sha256_der(&der)?;
            if self
                .match_fingerprint
                .as_deref()
                .is_none_or(|want| want.eq_ignore_ascii_case(fingerprint.as_str()))
            {
                return Ok(DeviceCert { fingerprint });
            }
            prev = next;
            std::mem::forget(current);
        }

        Err(KeystoreError::NotFound(
            match self.match_fingerprint.as_deref() {
                Some(fp) => format!("no certificate in MY store matched SHA-256 {fp}"),
                None => "MY certificate store is empty".to_string(),
            },
        ))
    }
}

fn cert_encoded_bytes(ctx: *const CERT_CONTEXT) -> Vec<u8> {
    if ctx.is_null() {
        return Vec::new();
    }
    // SAFETY: ctx was returned by CertEnumCertificatesInStore and is non-null
    // (checked above); pbCertEncoded points to cbCertEncoded bytes of valid DER
    // for this context's lifetime.
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
