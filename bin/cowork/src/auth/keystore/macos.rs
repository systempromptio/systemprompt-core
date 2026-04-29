use super::{DeviceCert, DeviceCertSource, KeystoreError, sha256_der};
use security_framework::item::{ItemClass, ItemSearchOptions, Limit, Reference, SearchResult};
use std::env;

pub struct MacOsKeystore {
    label: Option<String>,
}

impl MacOsKeystore {
    pub fn new() -> Self {
        Self {
            label: env::var("SP_COWORK_DEVICE_CERT_LABEL").ok(),
        }
    }
}

impl DeviceCertSource for MacOsKeystore {
    fn load(&self) -> Result<DeviceCert, KeystoreError> {
        let Some(label) = self.label.as_deref() else {
            return Err(KeystoreError::NotConfigured(
                "SP_COWORK_DEVICE_CERT_LABEL unset; set to the Keychain label of the device \
                 certificate",
            ));
        };

        let mut opts = ItemSearchOptions::new();
        opts.class(ItemClass::certificate())
            .label(label)
            .load_refs(true)
            .limit(Limit::Max(1));

        let results = opts
            .search()
            .map_err(|e| KeystoreError::Other(format!("keychain search failed: {e}")))?;

        for result in results {
            if let SearchResult::Ref(Reference::Certificate(cert)) = result {
                let der = cert.to_der();
                if der.is_empty() {
                    return Err(KeystoreError::Other(
                        "keychain returned empty certificate data".into(),
                    ));
                }
                return Ok(DeviceCert {
                    fingerprint: sha256_der(&der)?,
                });
            }
        }

        Err(KeystoreError::NotFound(format!(
            "no certificate with label {label:?} found in keychain"
        )))
    }
}

pub fn platform_source() -> Box<dyn DeviceCertSource> {
    Box::new(MacOsKeystore::new())
}
