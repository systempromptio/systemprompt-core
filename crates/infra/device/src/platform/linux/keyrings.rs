//! The two keyrings of a Linux user, in the order they are preferred: the
//! Secret Service of the desktop session, then the kernel keyring of the
//! session.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::io;
use std::sync::Arc;

use crate::platform::CredentialStore;
use crate::platform::keyring::{KeyringStore, map_keyring};
use crate::secure::SecureError;

const PROBE_SERVICE: &str = "systemprompt-device keyring probe";
const PROBE_ACCOUNT: &str = "probe";

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub(super) enum Keyring {
    SecretService,
    Kernel,
}

impl Keyring {
    pub(super) const PREFERRED: [Self; 2] = [Self::SecretService, Self::Kernel];

    pub(super) const fn name(self) -> &'static str {
        match self {
            Self::SecretService => "secret-service",
            Self::Kernel => "kernel-keyring",
        }
    }

    pub(super) fn open(self) -> Result<KeyringStore, SecureError> {
        let store: Arc<keyring_core::CredentialStore> = match self {
            Self::SecretService => {
                dbus_secret_service_keyring_store::Store::new().map_err(absent)?
            },
            Self::Kernel => linux_keyutils_keyring_store::Store::new().map_err(absent)?,
        };
        Ok(KeyringStore::new(self.name(), store))
    }
}

pub(super) fn keeps_ciphertext(error: SecureError) -> SecureError {
    match error {
        SecureError::Keystore { source } => SecureError::Unavailable { source },
        other => other,
    }
}

fn absent(error: keyring_core::Error) -> SecureError {
    keeps_ciphertext(map_keyring(error))
}

// Why: the kernel keyring store opens without touching the kernel, and a
// container's seccomp profile can refuse its system calls; one read of a key
// that is not there shows whether the session keyring answers at all.
pub(super) fn first_usable() -> Result<Arc<dyn CredentialStore>, SecureError> {
    let secret_service = match Keyring::SecretService.open() {
        Ok(store) => return Ok(Arc::new(store)),
        Err(error) => error,
    };
    let kernel = Keyring::Kernel.open().and_then(|store| {
        store.load(PROBE_SERVICE, PROBE_ACCOUNT)?;
        Ok(store)
    });
    match kernel {
        Ok(store) => {
            tracing::warn!(
                error = %secret_service,
                "no Secret Service on this machine; credentials go to the kernel keyring and do not survive a reboot"
            );
            Ok(Arc::new(store))
        },
        Err(kernel) => Err(SecureError::Unavailable {
            source: io::Error::other(format!(
                "no Secret Service ({secret_service}) and no kernel keyring ({kernel})"
            )),
        }),
    }
}
