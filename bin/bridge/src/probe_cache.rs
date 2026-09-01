//! Per-process memo of slow host-presence probes.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use std::collections::HashMap;
use std::sync::Mutex;
use std::time::{Duration, Instant};

const CONCLUSIVE_TTL: Duration = Duration::from_secs(300);
const INCONCLUSIVE_TTL: Duration = Duration::from_secs(15);

/// Remembers whether a Start-menu entry was present, per display name.
///
/// `Get-StartApps` cold-starts PowerShell (seconds per call) and the host
/// probe runs every 30 seconds, so the answer is held for a while: longer when
/// it was conclusive, briefly when the query itself failed.
/// What a Start-menu probe concluded, if anything.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum StartMenuPresence {
    Present,
    Absent,
    Inconclusive,
}

impl StartMenuPresence {
    #[must_use]
    pub const fn from_probe(present: Option<bool>) -> Self {
        match present {
            Some(true) => Self::Present,
            Some(false) => Self::Absent,
            None => Self::Inconclusive,
        }
    }

    #[must_use]
    pub const fn as_probe(self) -> Option<bool> {
        match self {
            Self::Present => Some(true),
            Self::Absent => Some(false),
            Self::Inconclusive => None,
        }
    }
}

#[derive(Debug, Default)]
pub struct StartMenuCache {
    entries: Mutex<HashMap<String, (StartMenuPresence, Instant)>>,
}

impl StartMenuCache {
    #[must_use]
    pub fn lookup(&self, display_name: &str) -> Option<StartMenuPresence> {
        let (presence, at) = {
            let map = self.entries.lock().ok()?;
            *map.get(display_name)?
        };
        let ttl = if presence == StartMenuPresence::Inconclusive {
            INCONCLUSIVE_TTL
        } else {
            CONCLUSIVE_TTL
        };
        (at.elapsed() < ttl).then_some(presence)
    }

    pub fn record(&self, display_name: &str, presence: StartMenuPresence) {
        if let Ok(mut map) = self.entries.lock() {
            map.insert(display_name.to_owned(), (presence, Instant::now()));
        }
    }
}
