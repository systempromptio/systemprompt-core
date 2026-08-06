//! Profile-level overrides for the services manifest tree.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(Debug, Clone, Copy, Default, Serialize, Deserialize, schemars::JsonSchema)]
#[serde(deny_unknown_fields)]
pub struct ServicesProfileConfig {
    #[serde(default)]
    pub port_offset: u16,
}

impl ServicesProfileConfig {
    #[must_use]
    pub const fn is_identity(&self) -> bool {
        self.port_offset == 0
    }
}
