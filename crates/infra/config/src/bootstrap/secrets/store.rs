//! [`SecretStore`] over the live [`SecretsService`]: the handle extension code
//! keeps past router build, reading the current generation on every call.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::time::Duration;

use systemprompt_traits::{
    ExtensionSecret, ReloadOutcome, SECRET_SEED_BYTES, SecretStore, SecretStoreError, SecretsStatus,
};

use super::SecretsService;

#[async_trait::async_trait]
impl SecretStore for SecretsService {
    fn secret(&self, name: &str) -> Option<ExtensionSecret> {
        self.get()
            .custom_secret(name)
            .map(|value| ExtensionSecret::new(value.clone()))
    }

    fn key_names(&self) -> Result<Vec<String>, SecretStoreError> {
        Self::key_names(self).map_err(|e| SecretStoreError::new("listing the secret keys", e))
    }

    fn manifest_signing_secret_seed(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError> {
        Self::manifest_signing_secret_seed(self)
            .map_err(|e| SecretStoreError::new("decoding the manifest signing seed", e))
    }

    fn encryption_master_key(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError> {
        Self::encryption_master_key(self)
            .map_err(|e| SecretStoreError::new("decoding the encryption master key", e))
    }

    fn status(&self) -> SecretsStatus {
        Self::status(self)
    }

    fn refresh_interval(&self) -> Option<Duration> {
        Self::refresh_interval(self)
    }

    async fn reload(&self) -> Result<ReloadOutcome, SecretStoreError> {
        Self::reload(self)
            .await
            .map_err(|e| SecretStoreError::new("reloading the secrets", e))
    }
}
