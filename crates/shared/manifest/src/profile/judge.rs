//! Whether closed conversations are scored without anyone asking.
//!
//! The judge itself is a services concern; this profile block only decides if
//! the scheduler queues every closed conversation for scoring. Manual triggers
//! from the console work regardless, so a staging environment can be scored one
//! conversation at a time while production scores everything.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct JudgeProfile {
    #[serde(default)]
    pub automatic: bool,
}
