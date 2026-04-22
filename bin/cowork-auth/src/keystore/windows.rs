use super::{DeviceCert, DeviceCertSource, sha256_der};
use std::env;
use std::ptr;
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
    fn open_my() -> Result<Self, String> {
        let name: Vec<u16> = "MY\0".encode_utf16().collect();
        let handle = unsafe { CertOpenSystemStoreW(0, name.as_ptr()) };
        if handle.is_null() {
            return Err("CertOpenSystemStoreW(MY) returned NULL".to_string());
        }
        Ok(Self(handle))
    }
}

impl Drop for StoreHandle {
    fn drop(&mut self) {
        unsafe {
            CertCloseStore(self.0, 0);
        }
    }
}

struct CertHandle(*const CERT_CONTEXT);

impl Drop for CertHandle {
    fn drop(&mut self) {
        if !self.0.is_null() {
            unsafe {
                CertFreeCertificateContext(self.0);
            }
        }
    }
}

impl DeviceCertSource for WindowsKeystore {
    fn load(&self) -> Result<DeviceCert, String> {
        let store = StoreHandle::open_my()?;
        let mut prev: *const CERT_CONTEXT = ptr::null();

        loop {
            let next = unsafe { CertEnumCertificatesInStore(store.0, prev) };
            if next.is_null() {
                break;
            }
            let current = CertHandle(next);
            let der = cert_encoded_bytes(current.0);
            let fingerprint = sha256_der(&der);
            if self
                .match_fingerprint
                .as_deref()
                .is_none_or(|want| want.eq_ignore_ascii_case(&fingerprint))
            {
                return Ok(DeviceCert { fingerprint });
            }
            prev = next;
            std::mem::forget(current);
        }

        Err(match self.match_fingerprint.as_deref() {
            Some(fp) => format!("no certificate in MY store matched SHA-256 {fp}"),
            None => "MY certificate store is empty".to_string(),
        })
    }
}

fn cert_encoded_bytes(ctx: *const CERT_CONTEXT) -> Vec<u8> {
    if ctx.is_null() {
        return Vec::new();
    }
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
