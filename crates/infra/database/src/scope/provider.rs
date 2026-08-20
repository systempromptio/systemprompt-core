//! The [`ConnectionScopeProvider`] trait and its setting/error types.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::sync::Arc;

use systemprompt_models::RequestScope;

#[derive(Debug, thiserror::Error)]
pub enum ScopeError {
    #[error("invalid scope setting key '{key}': must be a dotted custom-GUC name")]
    InvalidKey { key: String },
    #[error("scope provider failed: {0}")]
    Provider(String),
}

/// One transaction-local setting to apply on a scoped transaction.
///
/// Injection-safe by construction: the key is validated to the custom-GUC
/// grammar at construction, and both key and value are bound as parameters to
/// `SELECT set_config($1, $2, true)` — never interpolated into SQL.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ScopeSetting {
    key: String,
    value: String,
}

impl ScopeSetting {
    pub fn new(key: impl Into<String>, value: impl Into<String>) -> Result<Self, ScopeError> {
        let key = key.into();
        if !is_custom_guc_name(&key) {
            return Err(ScopeError::InvalidKey { key });
        }
        Ok(Self {
            key,
            value: value.into(),
        })
    }

    #[must_use]
    pub fn key(&self) -> &str {
        &self.key
    }

    #[must_use]
    pub fn value(&self) -> &str {
        &self.value
    }
}

fn is_custom_guc_name(key: &str) -> bool {
    let mut segments = 0;
    for segment in key.split('.') {
        let mut chars = segment.chars();
        let Some(first) = chars.next() else {
            return false;
        };
        if !(first.is_ascii_alphabetic() || first == '_') {
            return false;
        }
        if !chars.all(|c| c.is_ascii_alphanumeric() || c == '_') {
            return false;
        }
        segments += 1;
    }
    segments >= 2
}

/// Translates a [`RequestScope`] into transaction-local settings.
///
/// `#[async_trait]`: consumed as `Arc<dyn ConnectionScopeProvider>` collected
/// from the inventory registry, so the trait must be dyn-compatible.
#[async_trait::async_trait]
pub trait ConnectionScopeProvider: Send + Sync {
    async fn scope_settings(&self, scope: &RequestScope) -> Result<Vec<ScopeSetting>, ScopeError>;
}

pub type SharedScopeProvider = Arc<dyn ConnectionScopeProvider>;
