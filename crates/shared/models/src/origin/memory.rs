//! The caller's team-memory preference, read from the
//! `x-systemprompt-memory` request header.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

/// Whether the caller allows team-memory recall on this request: `Off`
/// only for a header value of `off` in any case, `Default` otherwise.
#[derive(Debug, Clone, Copy, Default, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum MemoryPreference {
    #[default]
    Default,
    Off,
}

impl MemoryPreference {
    #[must_use]
    pub fn from_header(value: Option<&str>) -> Self {
        match value {
            Some(value) if value.eq_ignore_ascii_case("off") => Self::Off,
            _ => Self::Default,
        }
    }

    #[must_use]
    pub const fn is_off(self) -> bool {
        matches!(self, Self::Off)
    }
}
