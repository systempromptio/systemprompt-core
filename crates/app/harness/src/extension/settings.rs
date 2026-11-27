//! Each extension's own preferences, kept by the shell's preference store as
//! one JSON object per extension id and read here as the extension's own type.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::fmt;
use std::sync::Arc;

use harness_ids::ExtensionId;
use serde::Serialize;
use serde::de::DeserializeOwned;
use serde_json::{Map, Value};
use tokio::sync::watch;

/// The shell's preference store as extensions reach it.
///
/// Held as `dyn` by every context. `read` answers the extension's stored
/// object, `write` replaces it and persists it, and `changes` bumps whenever
/// any preference changed.
pub trait SettingsAccess: Send + Sync {
    // JSON: an extension's own preferences object, stored by the shell as written.
    fn read(&self, extension: &ExtensionId) -> Option<Map<String, Value>>;

    // JSON: an extension's own preferences object, stored by the shell as written.
    fn write(&self, extension: &ExtensionId, values: Map<String, Value>);

    fn changes(&self) -> watch::Receiver<u64>;
}

/// Why an extension's preferences could not be read or written.
#[derive(Debug, thiserror::Error)]
pub enum SettingsError {
    #[error("the preferences of extension {extension} do not match its settings type")]
    Decode {
        extension: ExtensionId,
        #[source]
        source: serde_json::Error,
    },
    #[error("the preferences of extension {extension} could not be encoded")]
    Encode {
        extension: ExtensionId,
        #[source]
        source: serde_json::Error,
    },
    #[error("the preferences of extension {extension} must encode as an object")]
    NotAnObject { extension: ExtensionId },
}

/// One extension's preferences, typed by the extension.
#[derive(Clone)]
pub struct ExtensionSettings {
    extension: ExtensionId,
    access: Arc<dyn SettingsAccess>,
}

impl ExtensionSettings {
    pub fn new(extension: ExtensionId, access: Arc<dyn SettingsAccess>) -> Self {
        Self { extension, access }
    }

    pub fn get<T: DeserializeOwned + Default>(&self) -> Result<T, SettingsError> {
        let Some(values) = self.access.read(&self.extension) else {
            return Ok(T::default());
        };
        serde_json::from_value(Value::Object(values)).map_err(|source| SettingsError::Decode {
            extension: self.extension.clone(),
            source,
        })
    }

    pub fn set<T: Serialize>(&self, value: &T) -> Result<(), SettingsError> {
        let encoded = serde_json::to_value(value).map_err(|source| SettingsError::Encode {
            extension: self.extension.clone(),
            source,
        })?;
        let Value::Object(values) = encoded else {
            return Err(SettingsError::NotAnObject {
                extension: self.extension.clone(),
            });
        };
        self.access.write(&self.extension, values);
        Ok(())
    }

    pub fn changes(&self) -> watch::Receiver<u64> {
        self.access.changes()
    }
}

impl fmt::Debug for ExtensionSettings {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.debug_struct("ExtensionSettings")
            .field("extension", &self.extension)
            .finish_non_exhaustive()
    }
}
