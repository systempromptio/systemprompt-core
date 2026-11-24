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
//! [`DeclaredSecrets`] is a member's read of that store: a custom secret by a
//! name the member declared at contribution **and** the operator granted it
//! in the profile, read by its exact name, and of those only a name no core
//! member, the profile or the platform references ([`SecretReferences`],
//! checked on every read so a reference added by a config reload is refused
//! from then on). It has no key listing and no reload. [`PINNED_SECRET_KEYS`]
//! is the one list of the platform's own sealing, signing and connection
//! material.
//!
//! [`SecretLookup`] is what a platform service (the in-process AI service, the
//! MCP client) resolves its credentials through, so a spawned child can run
//! the same service over its granted secrets alone.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::time::Duration;

use chrono::{DateTime, Utc};
use serde::Serialize;
use zeroize::Zeroize;

use crate::secret_pattern::SecretPattern;

pub const SECRET_SEED_BYTES: usize = 32;

pub const PINNED_SECRET_KEYS: &[&str] = &[
    "encryption_master_key",
    "oauth_at_rest_pepper",
    "manifest_signing_secret_seed",
    "database_url",
    "database_write_url",
    "database_session_url",
    "signing_key_pem",
    "signing_key_staged_pem",
    "signing_key_previous_keys",
    "jwt_secret",
];

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

/// The pepper OAuth secrets are hashed with at rest, sealed into the API
/// host's kernel state only; a child host never holds it.
#[derive(Clone)]
pub struct AtRestPepper(std::sync::Arc<str>);

impl AtRestPepper {
    #[must_use]
    pub fn new(pepper: &str) -> Self {
        Self(std::sync::Arc::from(pepper))
    }

    #[must_use]
    pub fn shared(&self) -> std::sync::Arc<str> {
        std::sync::Arc::clone(&self.0)
    }
}

impl fmt::Debug for AtRestPepper {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str("AtRestPepper(<redacted>)")
    }
}

/// The deployment's live secrets. Held as `Arc<dyn SecretStore>`, so it uses
/// `#[async_trait]` for `reload`; native `async fn` is not `dyn`-compatible.
#[async_trait::async_trait]
pub trait SecretStore: Send + Sync + fmt::Debug {
    fn secret(&self, name: &str) -> Option<ExtensionSecret>;

    fn secret_exact(&self, name: &str) -> Option<ExtensionSecret>;

    fn key_names(&self) -> Result<Vec<String>, SecretStoreError>;

    fn manifest_signing_secret_seed(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError>;

    fn encryption_master_key(&self) -> Result<[u8; SECRET_SEED_BYTES], SecretStoreError>;

    fn status(&self) -> SecretsStatus;

    fn refresh_interval(&self) -> Option<Duration>;

    async fn reload(&self) -> Result<ReloadOutcome, SecretStoreError>;
}

/// The secret names no install may read or declare.
///
/// A core member, the profile or the platform reads these. Held as
/// `Arc<dyn SecretReferences>`; a name is compared without regard to ASCII
/// case, as the store's lookup is.
pub trait SecretReferences: Send + Sync + fmt::Debug {
    fn references(&self, name: &str) -> bool;
}

/// One member's read of the live secrets: a custom secret by a name the
/// member declared and the operator granted it, read by that exact name, and
/// refused when any [`SecretReferences`] names it.
#[derive(Clone)]
pub struct DeclaredSecrets {
    store: std::sync::Arc<dyn SecretStore>,
    declared: std::sync::Arc<[SecretPattern]>,
    granted: std::sync::Arc<[SecretPattern]>,
    references: std::sync::Arc<dyn SecretReferences>,
}

impl DeclaredSecrets {
    #[must_use]
    pub fn over(
        store: std::sync::Arc<dyn SecretStore>,
        declared: std::sync::Arc<[SecretPattern]>,
        granted: std::sync::Arc<[SecretPattern]>,
        references: std::sync::Arc<dyn SecretReferences>,
    ) -> Self {
        Self {
            store,
            declared,
            granted,
            references,
        }
    }

    #[must_use]
    pub fn secret(&self, name: &str) -> Option<ExtensionSecret> {
        let declared = self.declared.iter().any(|pattern| pattern.covers(name));
        let granted = self.granted.iter().any(|pattern| pattern.covers(name));
        if !declared || !granted || self.references.references(name) {
            return None;
        }
        self.store.secret_exact(name)
    }
}

impl fmt::Debug for DeclaredSecrets {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("DeclaredSecrets")
            .field("declared", &self.declared)
            .field("granted", &self.granted)
            .finish_non_exhaustive()
    }
}

/// Name-to-value resolution over the secrets a platform service was handed.
///
/// The API holds the whole secrets document; a spawned child holds only the
/// granted set. Provider keys resolve by their provider name (`anthropic`),
/// custom secrets by their lowercase name.
pub trait SecretLookup: Send + Sync + fmt::Debug {
    fn lookup(&self, name: &str) -> Option<ExtensionSecret>;
}
