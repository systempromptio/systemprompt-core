//! Durable composite cursors include inventory and receipt changes without
//! inventing event history.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::Serialize;
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, schemars::JsonSchema)]
pub(super) struct Generation {
    pub(super) snapshots: i64,
    pub(super) inventory: i64,
    pub(super) installations: i64,
}
impl Generation {
    pub(super) fn parse(value: &str) -> Option<Self> {
        if value.len() > 96
            || value
                .split('.')
                .any(|part| part.is_empty() || !part.bytes().all(|byte| byte.is_ascii_digit()))
        {
            return None;
        }
        let parts: Vec<_> = value.split('.').map(str::parse::<i64>).collect();
        let [Ok(snapshots), Ok(inventory), Ok(installations)] = parts.as_slice() else {
            return None;
        };
        if *snapshots < 0 || *inventory < 0 || *installations < 0 {
            return None;
        }
        Some(Self {
            snapshots: *snapshots,
            inventory: *inventory,
            installations: *installations,
        })
    }
    pub(super) fn token(self) -> String {
        format!(
            "{}.{}.{}",
            self.snapshots, self.inventory, self.installations
        )
    }
    pub(super) fn requires_resync(self, previous: Self) -> bool {
        [
            (self.snapshots, previous.snapshots),
            (self.inventory, previous.inventory),
            (self.installations, previous.installations),
        ]
        .into_iter()
        .any(|(current, old)| current < old || current - old > 1)
    }
}
