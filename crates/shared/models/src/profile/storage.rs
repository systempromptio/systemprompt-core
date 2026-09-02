//! File storage backend selection for a profile.
//!
//! Copyright (c) systemprompt.io — Business Source License 1.1.
//! See <https://systemprompt.io> for licensing details.

use serde::{Deserialize, Serialize};

#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(rename_all = "lowercase")]
pub enum StorageBackend {
    #[default]
    Local,
}

/// Where user-visible files are written and whether the root is shared
/// between replicas.
///
/// `shared: true` declares that `paths.storage` is a mount every replica can
/// see; boot warns when the declaration and the observed mount disagree.
#[derive(
    Debug, Clone, Copy, PartialEq, Eq, Default, Serialize, Deserialize, schemars::JsonSchema,
)]
#[serde(deny_unknown_fields)]
pub struct StorageConfig {
    #[serde(default)]
    pub backend: StorageBackend,
    #[serde(default)]
    pub shared: bool,
}
