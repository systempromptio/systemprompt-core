//! The preference store of a process without a window: each extension's
//! preferences object held in memory for the life of the process.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::BTreeMap;

use parking_lot::Mutex;
use serde_json::{Map, Value};
use systemprompt_identifiers::engine::ExtensionId;
use tokio::sync::watch;

use crate::extension::SettingsAccess;

/// Extension preferences kept in memory; nothing is read from or written to
/// disk.
#[derive(Debug)]
pub struct MemorySettings {
    // JSON: each extension's own preferences object, kept as the extension wrote it.
    values: Mutex<BTreeMap<ExtensionId, Map<String, Value>>>,
    revision: watch::Sender<u64>,
}

impl Default for MemorySettings {
    fn default() -> Self {
        Self {
            values: Mutex::new(BTreeMap::new()),
            revision: watch::Sender::new(0),
        }
    }
}

impl SettingsAccess for MemorySettings {
    // JSON: an extension's own preferences object, kept as the extension wrote it.
    fn read(&self, extension: &ExtensionId) -> Option<Map<String, Value>> {
        self.values.lock().get(extension).cloned()
    }

    // JSON: an extension's own preferences object, kept as the extension wrote it.
    fn write(&self, extension: &ExtensionId, values: Map<String, Value>) {
        self.values.lock().insert(extension.clone(), values);
        self.revision
            .send_modify(|revision| *revision = revision.wrapping_add(1));
    }

    fn changes(&self) -> watch::Receiver<u64> {
        self.revision.subscribe()
    }
}
