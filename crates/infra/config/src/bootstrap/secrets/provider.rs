//! Remote secrets provider seam.
//!
//! A provider returns a [`SecretsDocument`] — the raw JSON object holding the
//! `secrets.json` shape — rather than a parsed [`Secrets`]. Keeping the
//! untyped document one step longer is what lets per-key overrides be merged
//! in before a single validation pass decides whether the result is usable.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::future::Future;

use systemprompt_models::secrets::Secrets;

use super::SecretsBootstrapError;

pub trait SecretsProvider {
    fn fetch(&self) -> impl Future<Output = Result<SecretsDocument, SecretsBootstrapError>> + Send;

    fn describe(&self) -> String;
}

#[derive(Debug, Clone, Default)]
pub struct SecretsDocument(serde_json::Map<String, serde_json::Value>);

impl SecretsDocument {
    #[must_use]
    pub const fn new(fields: serde_json::Map<String, serde_json::Value>) -> Self {
        Self(fields)
    }

    pub fn merge_field(&mut self, key: impl Into<String>, value: serde_json::Value) {
        self.0.insert(key.into(), value);
    }

    #[must_use]
    pub fn key_names(&self) -> Vec<String> {
        let mut names: Vec<String> = self.0.keys().cloned().collect();
        names.sort_unstable();
        names
    }

    #[must_use]
    pub fn is_empty(&self) -> bool {
        self.0.is_empty()
    }

    pub fn into_secrets(self) -> Result<Secrets, SecretsBootstrapError> {
        let body = serde_json::to_string(&serde_json::Value::Object(self.0)).map_err(|e| {
            SecretsBootstrapError::InvalidSecretsFile {
                message: e.to_string(),
            }
        })?;
        Secrets::parse(&body).map_err(|e| SecretsBootstrapError::InvalidSecretsFile {
            message: e.to_string(),
        })
    }
}
