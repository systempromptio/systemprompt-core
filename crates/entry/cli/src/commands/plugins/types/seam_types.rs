//! `plugins seams` output types: one row per admitted member and contribution
//! kind.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use schemars::JsonSchema;
use serde::{Deserialize, Serialize};
use systemprompt_identifiers::ExtensionId;

use super::extension_types::ExtensionStanding;

#[derive(Debug, Clone, Serialize, Deserialize, JsonSchema)]
pub struct SeamRow {
    pub member: ExtensionId,
    pub standing: ExtensionStanding,
    pub contribution: String,
    pub count: usize,
    pub entries: String,
}
