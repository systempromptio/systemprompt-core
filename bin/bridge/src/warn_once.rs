//! Repeat suppression for warnings raised from loops: a keyed fault warns
//! when its state changes and logs at debug while it persists.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Mutex;

/// The last reported state per key; built by `BridgeContext::start` and
/// shared with the loops that report through it. `changed` records a state
/// and answers true when it differs from the last one (a first sighting
/// included); `clear` forgets a key so its next fault warns again.
#[derive(Debug, Default)]
pub struct WarnOnce {
    seen: Mutex<HashMap<String, String>>,
}

impl WarnOnce {
    pub fn changed(&self, key: &str, state: &str) -> bool {
        let Ok(mut seen) = self.seen.lock() else {
            return true;
        };
        if seen.get(key).is_some_and(|last| last == state) {
            return false;
        }
        seen.insert(key.to_owned(), state.to_owned());
        true
    }

    pub fn clear(&self, key: &str) {
        if let Ok(mut seen) = self.seen.lock() {
            seen.remove(key);
        }
    }
}
