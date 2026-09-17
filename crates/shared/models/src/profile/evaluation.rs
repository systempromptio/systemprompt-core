//! Whether closed sessions are scored without anyone asking.
//!
//! The evaluation engine itself is a services concern; this profile block only
//! decides if the scheduler queues every closed session for scoring. Manual
//! triggers from the console work regardless, so a staging environment can be
//! scored one session at a time while production scores everything.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct EvaluationProfile {
    #[serde(default)]
    pub automatic: bool,
}
