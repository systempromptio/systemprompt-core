//! [`ExtensionSecret`]: one operator-provisioned secret handed to an
//! extension by name.
//!
//! Extensions read their credentials (an encryption master key, a webhook
//! signing secret) from the platform secrets document through the extension
//! context instead of the process environment, so a secret follows the
//! configured secrets source and its reloads. The value redacts itself in
//! `Debug` and is wiped from memory on drop.
//!
//! [`SecretStore`] is the deployment's live secrets service as extension code
//! holds it past router build: every read goes to the current generation, so a
//! handle kept by a handler sees reloads.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use zeroize::Zeroize;

pub const SECRET_SEED_BYTES: usize = 32;

pub struct ExtensionSecret {
    value: String,
}

impl ExtensionSecret {
    #[must_use]
    pub const fn new(value: String) -> Self {
        Self { value }
    }

    #[must_use]
    pub fn expose(&self) -> &str {
        &self.value
    }
}

impl fmt::Debug for ExtensionSecret {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("ExtensionSecret(<redacted>)")
    }
}

impl Drop for ExtensionSecret {
    fn drop(&mut self) {
        self.value.zeroize();
    }
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
pub struct ReloadOutcome {
    pub generation: u64,
    pub changed_keys: Vec<String>,
    pub swapped: bool,
}

#[derive(Debug, Clone, Serialize)]
pub struct SecretsStatus {
    pub source: String,
    pub generation: u64,
    pub initialised_at: DateTime<Utc>,
    pub last_reload_at: Option<DateTime<Utc>>,
    pub last_error: Option<String>,
}

#[derive(Debug, thiserror::Error)]
#[error("secret store: {message}")]
pub struct SecretStoreError {
    message: &'static str,
    #[source]
    source: Box<dyn std::error::Error + Send + Sync>,
}

impl SecretStoreError {
    #[must_use]
    pub fn new(
        message: &'static str,
        source: impl Into<Box<dyn std::error::Error + Send + Sync>>,
    ) -> Self {
        Self {
            message,
            source: source.into(),
        }
    }
}

/// The deployment's live secrets. Held as `Arc<dyn SecretStore>`, so it uses
/// `#[async_trait]` for `reload`; native `async fn` is not `dyn`-compatible.
#[async_trait::async_trait]
pub trait SecretStore: Send + Sync + fmt::Debug {
    fn secret(&self, name: &str) -> Option<ExtensionSecret>;

    fn key_names(&self) -> Result<Vec<String>, SecretStoreError>;

    fn manifest_signing_secret_seed(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError>;

    fn encryption_master_key(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError>;

    fn status(&self) -> SecretsStatus;

    fn refresh_interval(&self) -> Option<Duration>;

    async fn reload(&self) -> Result<ReloadOutcome, SecretStoreError>;
}
